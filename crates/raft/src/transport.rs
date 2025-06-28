use async_trait::async_trait;
use crate::log_entry::LogEntry;
use crate::rpc::{AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse};
#[async_trait]
pub trait Transport: Send + Sync { // TODO: why does transport know target_id?
    async fn send_append_entries(&self, target_id: u64, req: &AppendEntriesRequest) -> AppendEntriesResponse;
    async fn send_request_vote(&self, target_id: u64, req: &RequestVoteRequest) -> RequestVoteResponse;
}
