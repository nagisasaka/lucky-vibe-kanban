//! Short single-service admission barrier for filesystem mutations which cannot
//! participate in a SQLite transaction (manual Git/cleanup) and the in-memory
//! follow-up queue. NOT a business reservation or a model-execution lease.
//! Never hold this across model execution or host validation commands.
pub static MUTATIONS: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Same fixed identity on initial execution, continuation and recovery. This is
/// host execution context; the user's subsequent message remains unchanged.
pub fn worker_context(
    run: &db::models::integration::IntegrationRun,
    branch: &str,
    root: &std::path::Path,
) -> String {
    format!(
        "自動マージ {}: repository作業path={} / 候補branch=refs/heads/{} / target={} / 固定base={}。同じ候補で継続し、targetや採用対象を自動追随しない。integration_action inspectで保存された対象・参照資料・検証と公開履歴を確認する。",
        run.id,
        root.display(),
        branch,
        run.target_ref,
        run.payload.observed_target
    )
}
