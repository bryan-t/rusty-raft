use crate::state::State;
use crate::log_entry::LogEntry;

pub trait Storage: Send + Sync {
    fn save_state(&mut self, state: &State);
    fn load_state(&self) -> State;
    fn append_log(&mut self, entries: &[LogEntry]);
}