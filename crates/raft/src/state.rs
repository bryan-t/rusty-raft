use crate::log_entry::LogEntry;

#[derive(Debug)]
pub struct State {
    pub current_term: u64,
    pub voted_for: Option<u64>, // ID of the candidate

    pub commit_index: u64,
    pub last_applied: u64,
    pub last_log_index: u64, // TODO: remove last_log_index and last_log_term in state, and derive them instead
    pub last_log_term: u64
}

impl State {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            commit_index: 0,
            last_applied: 0,
            last_log_index: 0,
            last_log_term: 0
        }
    }
}