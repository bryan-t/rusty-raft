
use crate::role::Role;
use crate::transport::{AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse};
use crate::state::{State};
use crate::storage::Storage;
use crate::transport::Transport;
use tokio::time::{Duration, Instant};
use tokio::sync::RwLock;
use rand::Rng;



pub struct RaftNode<S: Storage, T: Transport> {
    id: u64,
    peers: Vec<u64>,
    role: Role,
    state: State,
    storage: S,
    transport: T,
    election_timeout: Duration,
    heartbeat_interval: Duration,
    last_heartbeat: Instant,
    state_lock: RwLock<()>, // Lock for state to ensure thread safety
    role_lock: RwLock<()>, // Lock for role to ensure thread safety
}

impl<S: Storage, T: Transport> RaftNode<S, T> { // TODO: representation of peers
    pub fn new(id: u64, peers: Vec<u64>, storage: S, transport: T) -> Self {
        let state = storage.load_state().unwrap_or_else(|_| State::new());
        
        Self {
            id,
            peers,
            role: Role::Follower,
            state,
            storage,
            transport,
            election_timeout: Duration::from_millis(150 + rand::rng().random_range(0..150)),
            heartbeat_interval: Duration::from_millis(50),
            last_heartbeat: Instant::now(),
            state_lock: RwLock::new(()), // Initialize the state lock
            role_lock: RwLock::new(()), // Initialize the role lock
        }
    }
    pub async fn start(&mut self) {
        // Initialize timers and start the main loop
        self.last_heartbeat = Instant::now();
        loop {
            self.tick().await;
            let role = self.get_role().await; // Get the current role
            match role {
                Role::Leader => {
                    // Send heartbeats
                    if self.last_heartbeat.elapsed() >= self.heartbeat_interval {
                        self.last_heartbeat = Instant::now();
                        let guard = self.state_lock.read().await; // Lock state for reading
                        let req = AppendEntriesRequest {
                                term: self.state.current_term,
                                leader_id: self.id,
                                prev_log_index: self.state.last_log_index(),
                                prev_log_term: self.state.last_log_term(),
                                entries: vec![], // Empty entries for heartbeat
                                leader_commit: self.state.commit_index,
                            };
                        drop(guard); // Release the lock before sending requests
                        for &peer in &self.peers { // TODO: call peers in parallel
                            let _ = self.transport.send_append_entries(peer, &req).await;
                        }
                    }
                }
                Role::Candidate => {
                    // Request votes from peers
                    if self.last_heartbeat.elapsed() >= self.election_timeout {
                        // TODO: pre-vote request
                        let mut votes = 1; // Candidate votes for itself
                        let guard = self.state_lock.read().await;
                        let req = RequestVoteRequest {
                                term: self.state.current_term,
                                candidate_id: self.id,
                                last_log_index: self.state.last_log_index(),
                                last_log_term: self.state.last_log_term(),
                            };
                        drop(guard); // Release the lock before sending requests
                        for &peer in &self.peers {
                            let res = self.transport.send_request_vote(peer, &req).await;
                            if res.vote_granted {
                                votes += 1;
                            }
                        }
                        if votes > (self.peers.len() as u64 + 1) / 2 {
                            self.become_leader(req.last_log_term, req.last_log_index).await;
                        } else {
                            self.become_follower(req.term).await; // Reset to follower if election fails
                        }
                    }
                }
                Role::Follower => {
                    // Check for election timeout
                    if self.last_heartbeat.elapsed() >= self.election_timeout {
                        self.become_candidate().await;
                    }
                } 
            }
        }
  
    }

    async fn tick(&mut self) {
        tokio::time::sleep(Duration::from_millis(10)).await; // TODO: configurable tick interval
    }

    pub async fn handle_append_entries(&mut self, req: AppendEntriesRequest) -> AppendEntriesResponse {
        let guard = self.state_lock.write().await; // Lock state for reading
        // Handle append entries requests
        if req.term < self.state.current_term {
            return AppendEntriesResponse {
                term: self.state.current_term,
                success: false,
                match_index: self.state.commit_index,
            };
        }

        if req.term > self.state.current_term {
            drop(guard); // Release the lock before mutable borrow
            self.become_follower(req.term).await;
            return AppendEntriesResponse {
                term: self.state.current_term,
                success: false,
                match_index: self.state.commit_index,
            };
        }

        // Check the previous log entry
        if req.prev_log_index > self.state.last_log_index() || 
           (req.prev_log_index > 0 && self.state.log[req.prev_log_index as usize - 1].term != req.prev_log_term) {
            return AppendEntriesResponse {
                term: self.state.current_term,
                success: false,
                match_index: self.state.commit_index,
            };
        }

        // Append new entries
        let start_index = req.prev_log_index + 1;
        for (i, entry) in req.entries.iter().enumerate() { // TODO: this should be operated thru the storage
            if start_index + i as u64 <= self.state.last_log_index() && 
               self.state.log[(start_index + i as u64) as usize].term != entry.term {
                // Conflict detected, truncate log
                self.state.log.truncate(start_index as usize);
                break;
            }
            if start_index + i as u64 > self.state.last_log_index() {
                self.state.log.push(entry.clone());
            }
        }

        // Update commit index
        if req.leader_commit > self.state.commit_index {
            let new_commit = std::cmp::min(req.leader_commit, self.state.last_log_index());
            self.state.commit_index = new_commit;
        }

        // Save state to storage
        self.storage.save_state(&self.state);

        AppendEntriesResponse {
            term: self.state.current_term,
            success: true,
            match_index: self.state.commit_index,
        }
    }

    pub async fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        // Handle request vote requests
        if req.term < self.state.current_term {
            return RequestVoteResponse {
                term: self.state.current_term,
                vote_granted: false,
            };
        }

        if req.term > self.state.current_term {
            self.become_follower(req.term).await; // TODO: thread safety
        }

        // Check if already voted for someone else
        if let Some(voted_for) = self.state.voted_for {
            if voted_for != req.candidate_id {
                return RequestVoteResponse {
                    term: self.state.current_term,
                    vote_granted: false,
                };
            }
        }

        // Check log consistency
        if req.last_log_index < self.state.last_log_index() ||
           (req.last_log_index == self.state.last_log_index() && req.last_log_term < self.state.last_log_term()) {
            return RequestVoteResponse {
                term: self.state.current_term,
                vote_granted: false,
            };
        }

        // Grant vote
        self.state.voted_for = Some(req.candidate_id);
        self.storage.save_state(&self.state);

        RequestVoteResponse {
            term: self.state.current_term,
            vote_granted: true,
        }
    }

    async fn become_follower(&mut self, term: u64) {
        let _guard = self.role_lock.write().await; // Lock role for writing
        self.role = Role::Follower;
        self.state.current_term = term;
        self.state.voted_for = None;
    }

    async fn become_candidate(&mut self) {
        let _guard = self.role_lock.write().await; // Lock role for writing
        let _state_guard = self.state_lock.write().await; // Lock state for writing
        self.role = Role::Candidate;
        self.state.current_term += 1;
        self.state.voted_for = Some(self.id);
        // Vote requests to peers are sent in the main loop when in Candidate role.
    }

    async fn become_leader(&mut self, vote_term: u64, vote_last_log_index: u64) {
        let _guard = self.role_lock.write().await; // Lock role for writing
        let _state_guard = self.state_lock.write().await; // Lock state for writing
        if vote_term != self.state.current_term || 
           vote_last_log_index != self.state.last_log_index() || self.role != Role::Candidate {
            return; // Ignore. It means we are not in the correct state to become leader. Maybe an append entries request was received.
        }
        self.role = Role::Leader;
        self.state.voted_for = None; // Clear voted_for when becoming leader
        self.state.commit_index = self.state.last_log_index(); // Commit all entries up to the last log index
        self.storage.save_state(&self.state); // Save state to storage
        // Note: Initial empty AppendEntries (heartbeats) will be sent in the main loop
    }
    async fn get_role(&self) -> Role {
        let _guard = self.role_lock.read().await; // Lock role for reading
        self.role.clone() // Return a clone of the role
    }
}