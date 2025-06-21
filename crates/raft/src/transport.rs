use async_trait::async_trait;

#[async_trait]
pub trait Transport: Send + Sync {
    async fn send_append_entries(&self, target_id: u64, req: AppendEntriesRequest) -> AppendEntriesResponse;
    async fn send_request_vote(&self, target_id: u64, req: RequestVoteRequest) -> RequestVoteResponse;
}

// Define your RPC request/response structs
pub struct AppendEntriesRequest { /* ... */ }
pub struct AppendEntriesResponse { /* ... */ }

pub struct RequestVoteRequest { /* ... */ }
pub struct RequestVoteResponse { /* ... */ }