//! MCP argument DTOs shared by stdio and remote transports.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::*;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterListArgs {
    pub project_id: Uuid,
    #[serde(default)]
    pub page: SavedFilterPage,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterReadArgs {
    pub project_id: Uuid,
    pub saved_filter_id: Uuid,
    #[serde(default)]
    pub page: SavedFilterPage,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterCreateArgs {
    pub project_id: Uuid,
    pub input: CreateSavedFilterRequest,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterUpdateArgs {
    pub project_id: Uuid,
    pub saved_filter_id: Uuid,
    pub input: UpdateSavedFilterRequest,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterDeleteArgs {
    pub project_id: Uuid,
    pub saved_filter_id: Uuid,
    pub input: DeleteSavedFilterRequest,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedFilterEvaluateArgs {
    pub project_id: Uuid,
    pub saved_filter_id: Uuid,
    pub request_id: Uuid,
    #[serde(default)]
    pub retry_failed: bool,
    #[serde(default)]
    pub page: SavedFilterPage,
}
