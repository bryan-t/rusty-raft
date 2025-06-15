use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Follower,
    Candidate,
    Leader,
}

#[derive(Debug)]
pub struct RaftServer {
    pub id: u64,
    pub state: State,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub peers: Vec<u64>,
    pub election_timeout: Duration,
    pub last_heartbeat: Instant,
}

impl RaftServer {
    pub fn new(id: u64, peers: Vec<u64>, election_timeout: Duration) -> Self {
        RaftServer {
            id,
            state: State::Follower,
            current_term: 0,
            voted_for: None,
            peers,
            election_timeout,
            last_heartbeat: Instant::now(),
        }
    }

    pub fn run(self: Arc<Mutex<Self>>) {
        let server = self.clone();
        thread::spawn(move || loop {
            {
                let mut srv = server.lock().unwrap();
                match srv.state {
                    State::Follower => {
                        if srv.last_heartbeat.elapsed() > srv.election_timeout {
                            srv.state = State::Candidate;
                        }
                    }
                    State::Candidate => {
                        srv.current_term += 1;
                        srv.voted_for = Some(srv.id);
                        // In a real implementation, send RequestVote RPCs here
                        // For now, just become leader for demonstration
                        srv.state = State::Leader;
                    }
                    State::Leader => {
                        // In a real implementation, send AppendEntries (heartbeat) RPCs here
                        srv.last_heartbeat = Instant::now();
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        });
    }

    pub fn receive_heartbeat(&mut self, term: u64) {
        if term >= self.current_term {
            self.current_term = term;
            self.state = State::Follower;
            self.last_heartbeat = Instant::now();
        }
    }
}