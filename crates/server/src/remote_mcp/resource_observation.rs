//! On-demand Linux statistics only. No process inventory, shell, or caller paths.
use std::{io::Read, sync::Arc, time::Duration};

use serde_json::{Value, json};
use tokio::sync::{Semaphore, mpsc};

use super::read::{arguments, definition};

mod linux;

const READ_LIMIT: u64 = 64 * 1024;
const RESPONSE_LIMIT: usize = 8 * 1024;
const DEADLINE: Duration = Duration::from_secs(2);
static WORKERS: std::sync::LazyLock<Arc<Semaphore>> =
    std::sync::LazyLock::new(|| Arc::new(Semaphore::new(2)));

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {}

pub(super) fn tools() -> Vec<rmcp::model::Tool> {
    vec![definition::<Args>(
        "get_resource_observation",
        "Observe LVK server-visible Linux memory, swap, online CPU capacity, load averages and a roughly 500ms CPU sample, with separately labelled cgroup v2 limits. Read-only, bounded and on-demand; unknown fields retain errors. Not physical-host visibility, reserved capacity, admission or a prediction of future execution safety. No arguments.",
    )]
}

fn read(path: &'static str) -> Result<String, &'static str> {
    if !cfg!(target_os = "linux") {
        return Err("unsupported_platform");
    }
    let file = std::fs::File::open(path).map_err(|_| "unreadable")?;
    read_bounded(file)
}

fn read_bounded(reader: impl Read) -> Result<String, &'static str> {
    let mut bytes = Vec::new();
    reader
        .take(READ_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unreadable")?;
    if bytes.len() > READ_LIMIT as usize {
        return Err("size_limit");
    }
    String::from_utf8(bytes).map_err(|_| "invalid_encoding")
}

fn field<T: serde::Serialize>(result: Result<T, &'static str>) -> Value {
    match result {
        Ok(value) => json!({"status":"known", "value":value}),
        Err(error) => json!({"status":"unknown", "error":error}),
    }
}

fn initial() -> Value {
    let unknown = field::<u64>(Err("not_observed"));
    json!({
        "schema_version":1, "timestamp":chrono::Utc::now(),
        "scope":{"statistics":"system-visible", "environment":"unknown",
            "physical_host_visibility":"unknown"},
        "system_visible":{
            "memory_total_bytes":unknown, "memory_available_bytes":unknown,
            "swap_total_bytes":unknown, "swap_used_bytes":unknown,
            "online_cpu_count":unknown, "load_average":unknown,
            "cpu_usage":{"percent":unknown, "sampling_duration_ms":null, "scope":"system-visible", "iowait_treated_as_idle":true}},
        "cgroup": {"status":"unknown", "error":"not_observed"},
        "completion":"partial", "errors":[],
        "limitations":"Point-in-time observations, not reservations or future safety guarantees. Guest/container visibility cannot establish physical-host capacity. A frozen LVK server cannot answer this tool."
    })
}

fn snapshot(
    send: &mpsc::Sender<(&'static str, Value)>,
    read: impl Fn(&'static str) -> Result<String, &'static str>,
) {
    let mem = read("/proc/meminfo");
    for (key, name) in [
        ("memory_total_bytes", "MemTotal"),
        ("memory_available_bytes", "MemAvailable"),
        ("swap_total_bytes", "SwapTotal"),
    ] {
        let _ = send.blocking_send((
            key,
            field(
                mem.as_deref()
                    .map_err(|e| *e)
                    .and_then(|s| linux::memory(s, name)),
            ),
        ));
    }
    let swap = mem.as_deref().map_err(|e| *e).and_then(|s| {
        linux::memory(s, "SwapTotal")?
            .checked_sub(linux::memory(s, "SwapFree")?)
            .ok_or("invalid_swap")
    });
    let _ = send.blocking_send(("swap_used_bytes", field(swap)));
    let cpus = read("/sys/devices/system/cpu/online").and_then(|s| linux::cpuset(&s));
    let _ = send.blocking_send(("online_cpu_count", field(cpus)));
    let _ = send.blocking_send((
        "load_average",
        field(read("/proc/loadavg").and_then(|s| linux::load(&s))),
    ));

    // Only the fixed, namespace-root v2 mount is supported. Never guess that a
    // mount root belongs to this process when membership names a nested group.
    let group = read("/proc/self/cgroup")
        .and_then(|s| linux::root_v2(&s))
        .and_then(|()| read("/proc/self/mountinfo").and_then(|s| linux::root_mount(&s)));
    match group {
        Err(error) => {
            let _ = send.blocking_send(("cgroup", json!({"status":"unknown", "error":error})));
        }
        Ok(()) => {
            let unknown = field::<u64>(Err("not_observed"));
            let _ = send.blocking_send((
                "cgroup",
                json!({"status":"observed", "version":2, "scope":"visible_cgroup_namespace_root",
                "memory_current_bytes":unknown, "memory_max_bytes":unknown,
                "cpu_quota_cores":unknown, "cpuset_cpu_count":unknown,
                "effective_cpu_bound_cores":unknown, "hidden_ancestor_limits":"unknown"}),
            ));
            let current = read("/sys/fs/cgroup/memory.current").and_then(|s| linux::number(&s));
            let _ = send.blocking_send(("cgroup.memory_current_bytes", field(current)));
            let max = read("/sys/fs/cgroup/memory.max").and_then(|s| linux::limit(&s));
            let _ = send.blocking_send(("cgroup.memory_max_bytes", limit_field(max)));
            let quota = read("/sys/fs/cgroup/cpu.max").and_then(|s| linux::quota(&s));
            let _ = send.blocking_send(("cgroup.cpu_quota_cores", limit_field(quota)));
            let set = read("/sys/fs/cgroup/cpuset.cpus.effective").and_then(|s| linux::cpuset(&s));
            let _ = send.blocking_send(("cgroup.cpuset_cpu_count", field(set)));
            let _ = send.blocking_send((
                "cgroup.effective_cpu_bound_cores",
                field(linux::bound(cpus, quota, set)),
            ));
        }
    }
}

fn limit_field<T: serde::Serialize>(result: Result<Option<T>, &'static str>) -> Value {
    match result {
        Ok(None) => json!({"status":"unlimited"}),
        Ok(Some(value)) => field(Ok(value)),
        Err(error) => field::<T>(Err(error)),
    }
}

async fn receive(
    mut receiver: mpsc::Receiver<(&'static str, Value)>,
    deadline: Duration,
    mut result: Value,
) -> Value {
    let complete = tokio::time::timeout(deadline, async {
        while let Some((name, value)) = receiver.recv().await {
            if let Some(key) = name.strip_prefix("cgroup.") {
                result["cgroup"][key] = value;
            } else if name == "cgroup" {
                result["cgroup"] = value;
            } else if name == "done" {
                result["completion"] = json!("complete");
            } else {
                result["system_visible"][name] = value;
            }
        }
    })
    .await;
    if complete.is_err() {
        result["errors"] = json!(["timeout"]);
    } else if result["completion"] != "complete" {
        result["errors"] = json!(["worker_failed"]);
    }
    result["completed_at"] = json!(chrono::Utc::now());
    result
}

pub(super) async fn call(args: Value) -> anyhow::Result<Value> {
    let _: Args = arguments(args)?;
    observe(WORKERS.clone()).await
}

async fn observe(workers: Arc<Semaphore>) -> anyhow::Result<Value> {
    collect(workers, DEADLINE, |send| {
        snapshot(&send, read);
        if send.is_closed() {
            return;
        }
        let first = read("/proc/stat").and_then(|s| linux::cpu(&s));
        let started = std::time::Instant::now();
        std::thread::sleep(Duration::from_millis(500));
        let last = read("/proc/stat").and_then(|s| linux::cpu(&s));
        let duration = started.elapsed();
        let usage = first.and_then(|a| last.and_then(|b| linux::usage(a, b)));
        let _ = send.blocking_send(("cpu_usage", sample(usage, duration)));
        let _ = send.blocking_send(("done", Value::Null));
    })
    .await
}

fn sample(usage: Result<f64, &'static str>, duration: Duration) -> Value {
    json!({"percent":field(usage), "sampling_duration_ms":duration.as_secs_f64()*1000.0,
        "scope":"system-visible", "iowait_treated_as_idle":true})
}

async fn collect(
    workers: Arc<Semaphore>,
    deadline: Duration,
    job: impl FnOnce(mpsc::Sender<(&'static str, Value)>) + Send + 'static,
) -> anyhow::Result<Value> {
    let permit = workers
        .try_acquire_owned()
        .map_err(|_| anyhow::anyhow!("Resource observation busy; retry later"))?;
    let result = initial();
    let (send, receiver) = mpsc::channel(16);
    // Dedicated threads, never Tokio's shared blocking pool. The worker owns
    // the permit even if the caller disconnects/times out; stalled kernel I/O
    // cannot cause an unbounded accumulation of threads on subsequent calls.
    std::thread::Builder::new()
        .name("lvk-resource-read".into())
        .spawn(move || {
            let _permit = permit;
            job(send);
        })?;
    let result = receive(receiver, deadline, result).await;
    anyhow::ensure!(
        serde_json::to_vec(&result)?.len() <= RESPONSE_LIMIT,
        "Resource observation response size limit"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_read_rejects_oversize_invalid_encoding_and_io_errors() {
        assert_eq!(read_bounded(std::io::Cursor::new(b"123")), Ok("123".into()));
        assert_eq!(
            read_bounded(std::io::Cursor::new(vec![b'0'; READ_LIMIT as usize + 1])),
            Err("size_limit")
        );
        assert_eq!(
            read_bounded(std::io::Cursor::new(vec![255])),
            Err("invalid_encoding")
        );
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("private path"))
            }
        }
        assert_eq!(read_bounded(Broken), Err("unreadable"));
    }

    #[test]
    fn fixture_partial_snapshot_and_limits() {
        let (send, mut recv) = mpsc::channel(16);
        snapshot(&send, |path| match path {
            "/proc/meminfo" => Ok("MemTotal: 1024 kB\nSwapTotal: 8 kB\nSwapFree: 2 kB".into()),
            "/sys/devices/system/cpu/online" => Ok("0-7".into()),
            "/proc/loadavg" => Ok("NaN 2 3".into()),
            "/proc/self/cgroup" => Ok("0::/".into()),
            "/proc/self/mountinfo" => {
                Ok("38 29 0:32 / /sys/fs/cgroup ro - cgroup2 cgroup rw".into())
            }
            "/sys/fs/cgroup/memory.current" => Ok("123".into()),
            "/sys/fs/cgroup/memory.max" => Ok("max".into()),
            "/sys/fs/cgroup/cpu.max" => Ok("150000 100000".into()),
            "/sys/fs/cgroup/cpuset.cpus.effective" => Ok("0-3".into()),
            _ => Err("unreadable"),
        });
        drop(send);
        let mut fields = std::collections::HashMap::new();
        while let Ok((name, value)) = recv.try_recv() {
            fields.insert(name, value);
        }
        assert_eq!(fields["memory_total_bytes"]["value"], 1048576);
        assert_eq!(fields["memory_available_bytes"]["error"], "missing_field");
        assert_eq!(fields["swap_used_bytes"]["value"], 6144);
        assert_eq!(fields["load_average"]["error"], "invalid_load");
        assert_eq!(fields["cgroup.memory_max_bytes"]["status"], "unlimited");
        assert_eq!(fields["cgroup.effective_cpu_bound_cores"]["value"], 1.5);
    }

    #[tokio::test]
    async fn timeout_preserves_partial_values() {
        let (send, recv) = mpsc::channel(2);
        send.send(("online_cpu_count", field(Ok(4)))).await.unwrap();
        let result = receive(recv, Duration::from_millis(10), initial()).await;
        assert_eq!(result["system_visible"]["online_cpu_count"]["value"], 4);
        assert_eq!(result["errors"], json!(["timeout"]));
        assert_eq!(
            result["system_visible"]["swap_used_bytes"]["status"],
            "unknown"
        );
    }

    #[tokio::test]
    async fn response_size_is_bounded() {
        let result = collect(Arc::new(Semaphore::new(1)), DEADLINE, |send| {
            let _ = send.blocking_send(("load_average", json!("x".repeat(RESPONSE_LIMIT))));
        })
        .await;
        assert_eq!(
            result.unwrap_err().to_string(),
            "Resource observation response size limit"
        );
    }

    #[test]
    fn sample_reports_actual_duration_even_for_invalid_delta() {
        let sample = sample(Err("counter_regressed"), Duration::from_millis(527));
        assert_eq!(sample["percent"]["error"], "counter_regressed");
        assert_eq!(sample["sampling_duration_ms"], 527.0);
    }

    #[tokio::test]
    async fn timed_out_worker_keeps_its_slot_until_exit() {
        let workers = Arc::new(Semaphore::new(1));
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let result = collect(workers.clone(), Duration::from_millis(10), move |_send| {
            let _ = wait.recv();
        })
        .await
        .unwrap();
        assert_eq!(result["errors"], json!(["timeout"]));
        assert_eq!(workers.available_permits(), 0);
        assert!(observe(workers.clone()).await.is_err());
        drop(release);
        let _permit = tokio::time::timeout(Duration::from_secs(1), workers.acquire())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn rejects_arguments_and_bounds_concurrency() {
        assert!(call(json!({"path":"/etc/passwd"})).await.is_err());
        let workers = Arc::new(Semaphore::new(2));
        let _permits = workers.clone().acquire_many_owned(2).await.unwrap();
        assert!(
            observe(workers)
                .await
                .unwrap_err()
                .to_string()
                .contains("busy")
        );
    }

    #[test]
    fn read_only_catalog_and_explicit_unlimited() {
        let tool = super::super::tools()
            .into_iter()
            .find(|t| t.name == "get_resource_observation")
            .unwrap();
        assert_eq!(tool.input_schema["additionalProperties"], false);
        let value = serde_json::to_value(tool).unwrap();
        assert_eq!(value["annotations"]["readOnlyHint"], true);
        assert_eq!(
            value["_meta"]["securitySchemes"][0]["scopes"],
            json!(["lvk:read"])
        );
        let examples: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../scripts/chatgpt-mcp/examples.json"
        ))
        .unwrap();
        let example = examples
            .iter()
            .find(|case| case["tool"] == "get_resource_observation")
            .unwrap();
        let _: Args = arguments(example["arguments"].clone()).unwrap();
        assert_eq!(example["scopes"], json!(["lvk:read"]));
        assert_eq!(limit_field::<u64>(Ok(None))["status"], "unlimited");
        assert_eq!(limit_field::<u64>(Err("missing"))["status"], "unknown");
    }

    #[tokio::test]
    #[ignore = "explicit small live Linux smoke read only"]
    async fn live_smoke() {
        let result = call(json!({})).await.unwrap();
        assert_eq!(result["completion"], "complete");
        assert!(result.to_string().len() < 8192);
        assert!(
            result["system_visible"]["cpu_usage"]["sampling_duration_ms"]
                .as_f64()
                .unwrap()
                >= 500.0
        );
        println!("{result}");
    }
}
