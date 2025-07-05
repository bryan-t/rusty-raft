use crate::state::State;
use crate::log_entry::LogEntry;
use std::error::Error;

pub trait Storage: Send + Sync {
    // ---- State ----
    fn save_state(&mut self, state: &State) -> Result<(), Box<dyn Error>>;
    fn load_state(&self) -> Result<State, Box<dyn Error>>;

    // ---- Log ----
    fn append_log(&mut self, entries: &[LogEntry]) -> Result<(), Box<dyn Error>>;
    fn get_log(&self, index: u64) -> Result<Option<LogEntry>, Box<dyn Error>>;
    fn get_logs(&self, start: u64, end: u64) -> Result<Vec<LogEntry>, Box<dyn Error>>;
    fn truncate_suffix(&mut self, from_index: u64) -> Result<(), Box<dyn Error>>;
    fn last_log_entry(&self) -> Result<Option<LogEntry>, Box<dyn Error>>;
    fn term_at(&self, index: u64) -> Result<Option<u64>, Box<dyn Error>>;
}