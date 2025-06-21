use crate::{role::Role, state::State, storage::Storage, transport::Transport};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

pub struct RaftNode<T: Storage, U: Transport> {
    pub id: u64,
    pub peers: Vec<u64>,
    pub role: Role,
    pub state: State,
    pub storage: T,
    pub transport: U,
    pub election_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub last_heartbeat: Instant,
}

impl<T: Storage, U: Transport> RaftNode<T, U> {
    pub fn new(id: u64, peers: Vec<u64>, storage: T, transport: U) -> Self {
        let state = storage.load_state();
        Self {
            id,
            peers,
            role: Role::Follower,
            state,
            storage,
            transport,
            election_timeout: Duration::from_millis(150 + rand::random::<u64>() % 150),
            heartbeat_interval: Duration::from_millis(50),
            last_heartbeat: Instant::now(),
        }
    }

    pub async fn tick(&mut self) {
        // Election timeout logic
        // Heartbeat sending
        // Role transitions
    }

    pub async fn handle_append_entries(&mut self, req: AppendEntriesRequest) -> AppendEntriesResponse {
        // Handle replication from leader
    }

    pub async fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        // Handle vote requests
    }

    pub fn become_follower(&mut self, term: u64) {
        self.role = Role::Follower;
        self.state.current_term = term;
        self.state.voted_for = None;
    }

    pub fn become_candidate(&mut self) {
        self.role = Role::Candidate;
        self.state.current_term += 1;
        self.state.voted_for = Some(self.id);
        // Send vote requests to peers
    }

    pub fn become_leader(&mut self) {
        self.role = Role::Leader;
        // Send initial empty AppendEntries
    }
}