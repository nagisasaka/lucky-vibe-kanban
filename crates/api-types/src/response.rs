use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Response wrapper for mutation endpoints (create/update).
/// The legacy txid field remains a response revision for existing local consumers;
/// it is not an ElectricSQL acknowledgement.
#[derive(Debug, Serialize, Deserialize)]
pub struct MutationResponse<T> {
    pub data: T,
    pub txid: i64,
}

/// Response wrapper for delete endpoints.
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct DeleteResponse {
    pub txid: i64,
}
