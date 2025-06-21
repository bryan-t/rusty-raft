use crate::log_entry::LogEntry;

#[derive(Debug)]
pub struct State {
    pub current_term: u64,
    pub voted_for: Option<u64>, // ID of the candidate
    pub log: Vec<LogEntry>,
    pub commit_index: u64,
    pub last_applied: u64,
}

impl State {
    pub fn new() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            log: vec![],
            commit_index: 0,
            last_applied: 0,
        }
    }

    pub fn last_log_index(&self) -> u64 {
        (self.log.len() as u64).saturating_sub(1)
    }

    pub fn last_log_term(&self) -> u64 {
        self.log.last().map(|e| e.term).unwrap_or(0)
    }
}