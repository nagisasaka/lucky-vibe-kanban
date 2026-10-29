//! Verification-only records never enter source events, integrations or Wiki input.
use super::*;

pub const MAX_VERIFICATION_BYTES: usize = MAX_RECORD_BYTES as usize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationSource {
    pub source_commit: String,
    pub branch: String,
    pub clean: bool,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationStart {
    pub run_id: Uuid,
    pub repository_id: Uuid,
    pub workspace_id: Uuid,
    pub source: VerificationSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandExit {
    pub event_id: Uuid,
    pub run_attempt_id: Uuid,
    pub command: String,
    pub cwd: String,
    pub exit_code: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoChangeVerification {
    pub manifest: ChangeManifest,
    pub start: VerificationStart,
    pub end: VerificationSource,
    pub command_exits: Vec<CommandExit>,
}

/// Freeze declines too: a later draft edit cannot backfill a completed turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationOutcome {
    pub record: Option<NoChangeVerification>,
    pub reason: Option<String>,
}

impl VerificationOutcome {
    fn validate(&self, run_id: Uuid) -> io::Result<()> {
        if self.record.is_some() == self.reason.is_some() {
            return Err(invalid("Invalid verification outcome"));
        }
        if let Some(record) = &self.record {
            record.manifest.validate()?;
            if record.manifest.event_id != run_id
                || record.start.run_id != run_id
                || record.start.repository_id != record.manifest.repository_id
                || record.start.workspace_id != record.manifest.workspace_id
                || !record.manifest.changed_paths.is_empty()
                || record.manifest.base_commit != record.manifest.source_commit
                || !record.start.source.clean
                || !record.end.clean
                || record.start.source.source_commit != record.manifest.source_commit
                || record.end.source_commit != record.manifest.source_commit
                || record.start.source.branch != record.end.branch
            {
                return Err(invalid("Invalid no-change verification identity"));
            }
        }
        Ok(())
    }
}

impl RepositoryMemoryStore {
    pub fn verification_start(&self, run_id: Uuid) -> io::Result<Option<VerificationStart>> {
        let start: Option<VerificationStart> = self.read_json(
            &self
                .root
                .join("verification-starts")
                .join(format!("{run_id}.json")),
        )?;
        if start
            .as_ref()
            .is_some_and(|s| s.run_id != run_id || !commit_id(&s.source.source_commit))
        {
            return Err(invalid("Invalid verification start identity"));
        }
        Ok(start)
    }

    pub fn save_verification_start(&self, start: &VerificationStart) -> io::Result<()> {
        real_directory(&self.root.join("verification-starts"))?;
        self.publish(
            &self
                .root
                .join("verification-starts")
                .join(format!("{}.json", start.run_id)),
            start,
            false,
        )
    }

    pub fn verification(&self, run_id: Uuid) -> io::Result<Option<VerificationOutcome>> {
        let outcome: Option<VerificationOutcome> = self.read_json(
            &self
                .root
                .join("verifications")
                .join(format!("{run_id}.json")),
        )?;
        if let Some(outcome) = &outcome {
            outcome.validate(run_id)?;
        }
        Ok(outcome)
    }

    pub fn publish_verification(
        &self,
        run_id: Uuid,
        outcome: &VerificationOutcome,
    ) -> io::Result<()> {
        outcome.validate(run_id)?;
        real_directory(&self.root.join("verifications"))?;
        self.publish(
            &self
                .root
                .join("verifications")
                .join(format!("{run_id}.json")),
            outcome,
            false,
        )
    }
}
