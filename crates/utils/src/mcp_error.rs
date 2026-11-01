//! Safe MCP diagnostics shared by both transports. Classification is evidence,
//! not permission to repeat a side effect. Never put source errors in this type.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Validation,
    Conflict,
    Permission,
    NotFound,
    Busy,
    Unavailable,
    Internal,
    OutcomeUnknown,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub category: Category,
    pub outcome: Outcome,
    pub recovery: &'static str,
    pub message: &'static str,
    pub correlation_id: Uuid,
}

impl Diagnostic {
    pub fn new(category: Category, outcome: Outcome) -> Self {
        let (recovery, message) = if outcome == Outcome::Unknown {
            (
                "inspect_receipt_and_state",
                "The operation may have taken effect. Inspect the original receipt and target state before any new request. Reuse the original request ID and identical input only where the tool promises durable deduplication.",
            )
        } else {
            match category {
                Category::Validation => (
                    "correct_input",
                    "Invalid tool parameters; check the tool schema and correct the input.",
                ),
                Category::Conflict => (
                    "read_state_then_new_cas",
                    "Read current state and revision before deliberately submitting a new conditional operation.",
                ),
                Category::Permission => (
                    "obtain_permission",
                    "Authentication or permission is required. Preserve the original request identity.",
                ),
                Category::NotFound => (
                    "verify_target",
                    "The requested target is unavailable. Verify its identity and access.",
                ),
                Category::Busy => (
                    "wait_then_inspect",
                    "A resource is busy. Wait, then inspect the original receipt and state before retrying.",
                ),
                _ => (
                    "inspect_state",
                    "The operation failed. Inspect current state and use the correlation ID for diagnosis before retrying.",
                ),
            }
        };
        Self {
            category,
            outcome,
            recovery,
            message,
            correlation_id: Uuid::new_v4(),
        }
    }

    pub fn value(&self) -> Value {
        serde_json::to_value(self).expect("static MCP diagnostic")
    }
}

/// Only protocol/HTTP status facts are used, never server body text. A 5xx
/// response cannot establish whether a mutation committed before it failed.
pub fn http_category(status: u16) -> Category {
    match status {
        400 | 405 | 413 | 415 | 422 => Category::Validation,
        401 | 403 => Category::Permission,
        404 | 410 => Category::NotFound,
        409 | 412 => Category::Conflict,
        423 | 429 => Category::Busy,
        502..=504 => Category::Unavailable,
        _ => Category::Internal,
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for Diagnostic {}

/// Failure context is diagnostic data, not a second delivery of request text or
/// execution configuration. Keep IDs, state, revisions and partial-effect facts;
/// withhold raw paths and user-authored content. Success payloads are untouched.
pub fn redact_failure_context(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "path"
                        | "cwd"
                        | "executable"
                        | "purpose"
                        | "prompt"
                        | "instruction"
                        | "content"
                        | "body"
                        | "title"
                        | "description"
                        | "command"
                        | "args"
                        | "environment"
                        | "executor_action"
                ) && !value.is_null()
                {
                    *value = Value::String("[withheld]".into());
                } else {
                    redact_failure_context(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_failure_context(value);
            }
        }
        _ => {}
    }
}
