//! Pure bounded-input parsers. Errors never include source text.
pub(super) fn number(s: &str) -> Result<u64, &'static str> {
    let s = s.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid_number");
    }
    s.parse().map_err(|_| "invalid_number")
}

pub(super) fn memory(s: &str, key: &str) -> Result<u64, &'static str> {
    let mut matches = s
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| *name == key);
    let (_, value) = matches.next().ok_or("missing_field")?;
    if matches.next().is_some() {
        return Err("duplicate_field");
    }
    let words: Vec<_> = value.split_whitespace().collect();
    if words.len() != 2 || words[1] != "kB" {
        return Err("invalid_unit");
    }
    number(words[0])?.checked_mul(1024).ok_or("overflow")
}

pub(super) fn cpuset(s: &str) -> Result<u64, &'static str> {
    let mut count = 0u64;
    let mut previous = None;
    for range in s.trim().split(',') {
        let (start, end) = match range.split_once('-') {
            Some((a, b)) => (number(a)?, number(b)?),
            None => {
                let n = number(range)?;
                (n, n)
            }
        };
        if start > end || previous.is_some_and(|p| start <= p) {
            return Err("invalid_cpuset");
        }
        count = end
            .checked_sub(start)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| count.checked_add(n))
            .ok_or("overflow")?;
        previous = Some(end);
    }
    Ok(count)
}

pub(super) fn limit(s: &str) -> Result<Option<u64>, &'static str> {
    if s.trim() == "max" {
        Ok(None)
    } else {
        number(s).map(Some)
    }
}

pub(super) fn quota(s: &str) -> Result<Option<f64>, &'static str> {
    let values: Vec<_> = s.split_whitespace().collect();
    if values.len() != 2 {
        return Err("invalid_quota");
    }
    let period = number(values[1])?;
    if period == 0 {
        return Err("invalid_quota");
    }
    match limit(values[0])? {
        None => Ok(None),
        Some(0) => Err("invalid_quota"),
        Some(n) => Ok(Some(n as f64 / period as f64)),
    }
}

pub(super) fn bound(
    online: Result<u64, &'static str>,
    quota: Result<Option<f64>, &'static str>,
    set: Result<u64, &'static str>,
) -> Result<f64, &'static str> {
    let capacity = (online? as f64).min(set? as f64);
    Ok(quota?.map_or(capacity, |quota| capacity.min(quota)))
}

pub(super) fn root_v2(s: &str) -> Result<(), &'static str> {
    if s.trim() == "0::/" {
        Ok(())
    } else {
        Err("unsupported_cgroup_layout")
    }
}

pub(super) fn root_mount(s: &str) -> Result<(), &'static str> {
    let mut found = false;
    for line in s.lines() {
        let Some((mount, filesystem)) = line.split_once(" - ") else {
            continue;
        };
        let fields: Vec<_> = mount.split_whitespace().collect();
        if fields.get(4) != Some(&"/sys/fs/cgroup") {
            continue;
        }
        if found
            || fields.len() < 6
            || filesystem.split_whitespace().count() < 3
            || fields.get(3) != Some(&"/")
            || filesystem.split_whitespace().next() != Some("cgroup2")
        {
            return Err("unsupported_cgroup_mount");
        }
        found = true;
    }
    if found {
        Ok(())
    } else {
        Err("unsupported_cgroup_mount")
    }
}

pub(super) fn load(s: &str) -> Result<[f64; 3], &'static str> {
    let mut words = s.split_whitespace();
    let mut values = [0.0_f64; 3];
    for value in &mut values {
        *value = words
            .next()
            .ok_or("missing_field")?
            .parse()
            .map_err(|_| "invalid_load")?;
        if !value.is_finite() || *value < 0.0 {
            return Err("invalid_load");
        }
    }
    Ok(values)
}

pub(super) fn cpu(s: &str) -> Result<[u64; 8], &'static str> {
    let mut words = s.lines().next().ok_or("missing_field")?.split_whitespace();
    if words.next() != Some("cpu") {
        return Err("missing_field");
    }
    let mut counters = [0; 8];
    // Guest counters are already included in user/nice. Require all eight
    // modern Linux counters, rather than guessing missing fields are zero.
    for counter in &mut counters {
        *counter = number(words.next().ok_or("missing_field")?)?;
    }
    Ok(counters)
}

pub(super) fn usage(a: [u64; 8], b: [u64; 8]) -> Result<f64, &'static str> {
    let mut delta = [0u64; 8];
    for i in 0..8 {
        delta[i] = b[i].checked_sub(a[i]).ok_or("counter_regressed")?;
    }
    let total = delta
        .iter()
        .try_fold(0u64, |a, b| a.checked_add(*b))
        .ok_or("overflow")?;
    if total == 0 {
        return Err("no_cpu_delta");
    }
    let busy = total - delta[3] - delta[4];
    Ok(100.0 * busy as f64 / total as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_normal_missing_invalid_and_overflow() {
        let fixture = "MemTotal: 1024 kB\nMemAvailable: 256 kB\nSwapTotal: 0 kB\n";
        assert_eq!(memory(fixture, "MemTotal"), Ok(1048576));
        assert_eq!(memory(fixture, "SwapTotal"), Ok(0));
        assert_eq!(memory(fixture, "SwapFree"), Err("missing_field"));
        for value in ["-1 kB", "1 MB", "1", "18446744073709551615 kB", "hi kB"] {
            assert!(memory(&format!("MemTotal: {value}"), "MemTotal").is_err());
        }
        assert!(memory("MemTotal: 1 kB\nMemTotal: 2 kB", "MemTotal").is_err());
    }
    #[test]
    fn quota_cpuset_unlimited_and_unknown() {
        assert_eq!(limit("max\n"), Ok(None));
        assert_eq!(limit("0"), Ok(Some(0)));
        assert!(limit("").is_err());
        assert_eq!(quota("max 100000"), Ok(None));
        assert_eq!(quota("150000 100000"), Ok(Some(1.5)));
        assert_eq!(cpuset("0-3,8,10-11\n"), Ok(7));
        for set in ["", "1-0", "0-2,2", "1,0", "1-2-3", "-1"] {
            assert!(cpuset(set).is_err());
        }
        for q in ["max", "max 0", "0 100", "-1 100", "1 2 3"] {
            assert!(quota(q).is_err());
        }
        assert_eq!(bound(Ok(8), Ok(Some(1.5)), Ok(4)), Ok(1.5));
        assert_eq!(bound(Ok(8), Ok(None), Ok(4)), Ok(4.0));
        assert!(bound(Ok(8), Err("unreadable"), Ok(4)).is_err());
        assert!(root_v2("0::/nested").is_err());
        assert!(root_v2("2:cpu:/").is_err());
        assert!(root_v2("0::/\n").is_ok());
    }
    #[test]
    fn cgroup_mount_must_match_namespace_root_and_v2() {
        let root = "38 29 0:32 / /sys/fs/cgroup ro - cgroup2 cgroup rw";
        assert_eq!(root_mount(root), Ok(()));
        for mount in [
            "",
            "malformed",
            "38 29 0:32 /other /sys/fs/cgroup ro - cgroup2 cgroup rw",
            "38 29 0:32 / /elsewhere ro - cgroup2 cgroup rw",
            "38 29 0:32 / /sys/fs/cgroup ro - cgroup cgroup rw",
        ] {
            assert!(root_mount(mount).is_err());
        }
        assert!(root_mount(&format!("{root}\n{root}")).is_err());
    }

    #[test]
    fn cpu_deltas_exclude_guest_and_reject_regression() {
        let a = cpu("cpu 10 0 10 100 0 0 0 0 5 0\ncpu0 1").unwrap();
        let b = cpu("cpu 20 0 20 120 0 0 0 0 10 0").unwrap();
        assert_eq!(usage(a, b), Ok(50.0));
        assert!(usage(b, a).is_err());
        assert!(usage(a, a).is_err());
        assert!(cpu("cpu 1 2").is_err());
        assert!(usage([0; 8], [u64::MAX; 8]).is_err());
    }
    #[test]
    fn load_is_not_cpu_percent() {
        assert_eq!(load("1.25 2.5 3.75 1/20 999"), Ok([1.25, 2.5, 3.75]));
        for s in ["", "1 2", "NaN 1 2", "inf 1 2", "-1 2 3"] {
            assert!(load(s).is_err());
        }
    }
}
