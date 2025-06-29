
use std::io::Error;
use std::sync::{Arc, Mutex};

use crate::node;
use crate::role::Role;
use crate::state::State;
use crate::storage::Storage;
use crate::transport::Transport;
use crate::rpc::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
};
use futures::future::join_all;
use rand::Rng;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, Instant};

pub struct RaftNode<S: Storage + 'static, T: Transport + 'static> {
    core: Arc<Mutex<node::RaftNodeCore<S, T>>>,
    main_loop_handle: Option<std::thread::JoinHandle<()>>, // changed type
    stop_tx: mpsc::Sender<()>, // Sender to stop the main loops
    stop_rx: Option<mpsc::Receiver<()>>, // Receiver to stop the main loops
    append_entries_tx: mpsc::Sender<(AppendEntriesRequest, oneshot::Sender<AppendEntriesResponse>)>,
    request_vote_tx: mpsc::Sender<(RequestVoteRequest, oneshot::Sender<RequestVoteResponse>)>,
    append_entries_rx:
        Option<mpsc::Receiver<(AppendEntriesRequest, oneshot::Sender<AppendEntriesResponse>)>>,
    request_vote_rx:
        Option<mpsc::Receiver<(RequestVoteRequest, oneshot::Sender<RequestVoteResponse>)>>,
}

impl<S: Storage, T: Transport> RaftNode<S, T> {
    // TODO: representation of peers
    // TODO: id should not be passed, but rather generated if it is a new node
    pub fn new(id: u64, peers: Vec<u64>, storage: S, transport: T) -> Self {
        let (append_entries_tx, append_entries_rx) = mpsc::channel(64);
        let (request_vote_tx, request_vote_rx) = mpsc::channel(64);
        let (stop_tx, stop_rx) = mpsc::channel(1); // Channel to stop the main loop
        
        Self {
            core: Arc::new(Mutex::new(node::RaftNodeCore::new(id, peers, storage, transport))),
            main_loop_handle: None,
            stop_tx: stop_tx, // Store the sender to stop the main loop
            stop_rx: Some(stop_rx), // Store the receiver to stop the main loop
            append_entries_tx,
            request_vote_tx,
            append_entries_rx: Some(append_entries_rx),
            request_vote_rx: Some(request_vote_rx),
        }
    }

pub fn stop(&mut self) -> Result<(), Error> {
    // Send a stop signal to the main loop
    self.stop_tx
        .blocking_send(())
        .map_err(|_| Error::new(std::io::ErrorKind::Other, "Failed to send stop signal"))?;

    // Wait for the main loop thread to finish
    if let Some(handle) = self.main_loop_handle.take() {
        handle
            .join()
            .map_err(|_| Error::new(std::io::ErrorKind::Other, "Main loop thread panicked"))?;
    }

    Ok(())
}
    pub fn start(&mut self) {
        // Initialize timers and start the main loop

        let mut append_entries_rx = self.append_entries_rx.take().unwrap();
        let mut request_vote_rx = self.request_vote_rx.take().unwrap();
        let mut stop_rx = self.stop_rx.take().unwrap();
        let core = Arc::clone(&self.core);
        self.main_loop_handle = Some(std::thread::spawn(move || {
            // Each thread needs its own runtime
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            // Safety: only one main loop runs, and self is not mutably borrowed elsewhere
            rt.block_on(async move {
                let mut guard = core.lock().unwrap(); // Lock the core for the main loop
                guard.run(stop_rx, append_entries_rx, request_vote_rx).await;
            });
        }));
    }

    

    pub async fn handle_append_entries(
        &mut self,
        req: AppendEntriesRequest,
    ) -> AppendEntriesResponse {
        let (resp_tx, resp_rx) = oneshot::channel();
        // Send the request to the main loop via channel
        if let Err(_) = self.append_entries_tx.send((req, resp_tx)).await {
            // Channel closed, return failure
            return AppendEntriesResponse {
                term: 0,
                success: false,
                match_index: 0,
            };
        }
        // Await the response from the main loop
        match resp_rx.await {
            Ok(resp) => resp,
            Err(_) => AppendEntriesResponse {
                term: 0,
                success: false,
                match_index: 0,
            },
        }
    }

    pub async fn handle_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        let (resp_tx, resp_rx) = oneshot::channel();
        // Send the request to the main loop via channel
        if let Err(_) = self.request_vote_tx.send((req, resp_tx)).await {
            // Channel closed, return failure
            return RequestVoteResponse {
                term: 0,
                vote_granted: false,
            };
        }
        // Await the response from the main loop
        match resp_rx.await {
            Ok(resp) => resp,
            Err(_) => RequestVoteResponse {
                term: 0,
                vote_granted: false,
            },
        }
    }

    

    
}

pub struct RaftNodeCore<S: Storage + 'static, T: Transport + 'static> {
    id: u64,
    peers: Vec<u64>,
    role: Role,
    state: State,
    storage: S,
    transport: T,
    election_timeout: Duration,
    heartbeat_interval: Duration,
    last_heartbeat: Instant,
}

impl<S: Storage, T: Transport> RaftNodeCore<S, T> {
    pub fn new(id: u64, peers: Vec<u64>, storage: S, transport: T) -> Self {
        let state = storage.load_state().unwrap_or_else(|_| State::new());
        Self {
            id,
            peers,
            role: Role::Follower,
            state,
            storage,
            transport,
            election_timeout: Self::generate_election_timeout(),
            heartbeat_interval: Duration::from_millis(50),
            last_heartbeat: Instant::now(),
        }
    }
    fn generate_election_timeout() -> Duration {
        Duration::from_millis(150 + rand::rng().random_range(0..150))
    }

    fn become_follower(&mut self, term: u64) {
        self.role = Role::Follower;
        self.state.current_term = term;
        self.state.voted_for = None;
        self.election_timeout = Self::generate_election_timeout();
    }

    fn become_candidate(&mut self) {
        self.role = Role::Candidate;
        self.state.current_term += 1;
        self.state.voted_for = Some(self.id);
        self.election_timeout = Self::generate_election_timeout();
    }

    fn become_leader(&mut self) {
        self.role = Role::Leader;
        self.state.voted_for = None; // Clear voted_for when becoming leader
        self.state.commit_index = self.state.last_log_index(); // Commit all entries up to the last log index
        self.storage.save_state(&self.state); // Save state to storage
    }
    fn get_role(&self) -> Role {
        self.role.clone() // Return a clone of the role
    }

    // Internal processing for AppendEntries, called only from main_loop
    async fn process_append_entries(&mut self, req: AppendEntriesRequest) -> AppendEntriesResponse {
        // Handle append entries requests
        if req.term < self.state.current_term {
            return AppendEntriesResponse {
                term: self.state.current_term,
                success: false,
                match_index: self.state.commit_index,
            };
        }

        if req.term > self.state.current_term {
            self.become_follower(req.term);
            return AppendEntriesResponse {
                term: self.state.current_term,
                success: false,
                match_index: self.state.commit_index,
            };
        }

        // Check the previous log entry
        if req.prev_log_index > self.state.last_log_index()
            || (req.prev_log_index > 0
                && self.state.log[req.prev_log_index as usize - 1].term != req.prev_log_term)
        {
            return AppendEntriesResponse {
                term: self.state.current_term,
                success: false,
                match_index: self.state.commit_index,
            };
        }

        // Append new entries
        let start_index = req.prev_log_index + 1;
        for (i, entry) in req.entries.iter().enumerate() {
            // TODO: this should be operated thru the storage
            if start_index + i as u64 <= self.state.last_log_index()
                && self.state.log[(start_index + i as u64) as usize].term != entry.term
            {
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

        self.last_heartbeat = Instant::now(); // Update last heartbeat time

        // Save state to storage
        self.storage.save_state(&self.state);

        AppendEntriesResponse {
            term: self.state.current_term,
            success: true,
            match_index: self.state.commit_index,
        }
    }

    // Internal processing for RequestVote, called only from main_loop
    async fn process_request_vote(&mut self, req: RequestVoteRequest) -> RequestVoteResponse {
        // Handle request vote requests
        if req.term < self.state.current_term {
            return RequestVoteResponse {
                term: self.state.current_term,
                vote_granted: false,
            };
        }

        if req.term > self.state.current_term {
            self.become_follower(req.term); // TODO: thread safety
            return RequestVoteResponse {
                term: self.state.current_term,
                vote_granted: false,
            };
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
        if req.last_log_index < self.state.last_log_index()
            || (req.last_log_index == self.state.last_log_index()
                && req.last_log_term < self.state.last_log_term())
        {
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
    
    async fn run(
        &mut self,
        mut stop_rx: mpsc::Receiver<()>,
        mut append_entries_rx: mpsc::Receiver<(
            AppendEntriesRequest,
            oneshot::Sender<AppendEntriesResponse>,
        )>,
        mut request_vote_rx: mpsc::Receiver<(
            RequestVoteRequest,
            oneshot::Sender<RequestVoteResponse>,
        )>,
    ) {
        use tokio::select; // Not needed, select! is a macro
        loop {
            select! {
                Some((req, resp_tx)) = append_entries_rx.recv() => {
                    let resp = self.process_append_entries(req).await;
                    let _ = resp_tx.send(resp);
                }
                Some((req, resp_tx)) = request_vote_rx.recv() => {
                    let resp = self.process_request_vote(req).await;
                    let _ = resp_tx.send(resp);
                }
                Some(_) = stop_rx.recv() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    let role = self.get_role();
                    match role {
                        Role::Leader => {
                            // Send heartbeats
                            if self.last_heartbeat.elapsed() >= self.heartbeat_interval {
                                self.last_heartbeat = Instant::now();
                                let req = AppendEntriesRequest {
                                    term: self.state.current_term,
                                    leader_id: self.id,
                                    prev_log_index: self.state.last_log_index(),
                                    prev_log_term: self.state.last_log_term(),
                                    entries: vec![], // Empty entries for heartbeat
                                    leader_commit: self.state.commit_index,
                                };
                                let futures = self
                                    .peers
                                    .iter()
                                    .map(|&peer| self.transport.send_append_entries(peer, &req));
                                let _ = join_all(futures).await;
                            }
                        }
                        Role::Candidate => {
                            // Request votes from peers
                            if self.last_heartbeat.elapsed() >= self.election_timeout {
                                let mut votes = 1; // Candidate votes for itself
                                let req = RequestVoteRequest {
                                    term: self.state.current_term,
                                    candidate_id: self.id,
                                    last_log_index: self.state.last_log_index(),
                                    last_log_term: self.state.last_log_term(),
                                };
                                let futures = self
                                    .peers
                                    .iter()
                                    .map(|&peer| self.transport.send_request_vote(peer, &req));
                                let results = join_all(futures).await;
                                for res in results {
                                    if res.vote_granted {
                                        votes += 1;
                                    }
                                }
                                if votes > (self.peers.len() as u64 + 1) / 2 {
                                    self.become_leader()
                                } else {
                                    self.become_follower(req.term) // Reset to follower if election fails
                                }
                            }
                        }
                        Role::Follower => {
                            // Check for election timeout
                            if self.last_heartbeat.elapsed() >= self.election_timeout {
                                self.become_candidate();
                            }
                        }
                    }
                }
            }
        }

    }
}
