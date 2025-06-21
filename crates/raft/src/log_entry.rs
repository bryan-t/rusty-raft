#[derive(Debug, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub command: Vec<u8>, // Binary or JSON serialized command
}