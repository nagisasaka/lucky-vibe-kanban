//! LVK-owned delivery facts, never a reconstruction of provider system prompts.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::executors::{ExecutorError, codex::client::LogWriter};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstructionDelivery {
    pub id: String,
    pub route: String,
    pub role: String,
    pub reason: String,
    pub native_session_id: Option<String>,
    pub content: String,
    pub status: String,
}

impl InstructionDelivery {
    pub fn new(
        route: &str,
        role: &str,
        reason: &str,
        session: Option<&str>,
        content: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            route: route.into(),
            role: role.into(),
            reason: reason.into(),
            native_session_id: session.map(str::to_owned),
            content,
            status: "attempted".into(),
        }
    }

    pub async fn log(&self, writer: &LogWriter) -> Result<(), ExecutorError> {
        writer
            .log_raw(
                &serde_json::json!({
                    "type":"lvk_instruction_delivery", "lvk_instruction_delivery":self
                })
                .to_string(),
            )
            .await
    }
}

/// A small host receipt for an unchanged contract, not provider history analysis.
/// Unknown/legacy sessions and a changed contract always get the full injection.
#[derive(Clone, Debug)]
pub struct ResumeInstructions {
    pub allow_reduction: bool,
    pub current: String,
    pub fingerprint: String,
    pub directory: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
struct Receipt {
    fingerprint: Option<String>,
    last_compaction_id: Option<String>,
    last_compaction_fingerprint: Option<String>,
}

impl ResumeInstructions {
    pub fn new(root: &Path, fixed: &str, current: String) -> Self {
        Self {
            allow_reduction: true,
            current,
            fingerprint: format!("{:x}", Sha256::digest(fixed.as_bytes())),
            directory: root.join(".lvk/attachments/instruction-receipts-v1"),
        }
    }

    fn path(&self, session: &str) -> PathBuf {
        self.directory
            .join(format!("{:x}.json", Sha256::digest(session.as_bytes())))
    }

    fn read(&self, session: &str) -> Receipt {
        std::fs::read(self.path(session))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn matches(&self, session: &str) -> bool {
        self.read(session).fingerprint.as_ref() == Some(&self.fingerprint)
    }

    pub fn invalidate(&self, session: &str) -> std::io::Result<()> {
        let mut receipt = self.read(session);
        receipt.fingerprint = None;
        self.write(session, &receipt)
    }

    pub fn observe_compaction(&self, session: &str, id: &str) -> std::io::Result<()> {
        let mut receipt = self.read(session);
        if receipt.last_compaction_id.as_deref() == Some(id) {
            return Ok(());
        }
        receipt.fingerprint = None;
        receipt.last_compaction_id = Some(id.to_owned());
        receipt.last_compaction_fingerprint = None;
        self.write(session, &receipt)
    }

    pub fn compaction_matches(&self, session: &str, id: &str) -> bool {
        let receipt = self.read(session);
        receipt.last_compaction_id.as_deref() == Some(id)
            && receipt.last_compaction_fingerprint.as_ref() == Some(&self.fingerprint)
    }

    pub fn acknowledge_compaction(&self, session: &str) -> std::io::Result<()> {
        let mut receipt = self.read(session);
        receipt.fingerprint = Some(self.fingerprint.clone());
        receipt.last_compaction_fingerprint = Some(self.fingerprint.clone());
        self.write(session, &receipt)
    }

    pub fn acknowledge(&self, session: &str) -> std::io::Result<()> {
        let mut receipt = self.read(session);
        receipt.fingerprint = Some(self.fingerprint.clone());
        self.write(session, &receipt)
    }

    fn write(&self, session: &str, receipt: &Receipt) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let temporary = self.directory.join(format!("{}.tmp", Uuid::new_v4()));
        let bytes = serde_json::to_vec(receipt).map_err(std::io::Error::other)?;
        let result = std::fs::write(&temporary, bytes)
            .and_then(|_| std::fs::rename(&temporary, self.path(session)));
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipts_are_session_and_contract_scoped_and_uncertain_delivery_invalidates() {
        let dir = tempfile::tempdir().unwrap();
        let one = ResumeInstructions::new(dir.path(), "contract-1", "RUN=1".into());
        assert!(!one.matches("session-1"));
        one.acknowledge("session-1").unwrap();
        assert!(one.matches("session-1"));
        assert!(!one.matches("session-2"));
        let current = ResumeInstructions::new(dir.path(), "contract-1", "RUN=2".into());
        assert!(current.matches("session-1"));
        let changed = ResumeInstructions::new(dir.path(), "contract-2", "RUN=2".into());
        assert!(!changed.matches("session-1"));
        current.invalidate("session-1").unwrap();
        assert!(!one.matches("session-1"));
    }
}
