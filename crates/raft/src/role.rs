pub trait Role {
    /// Handle an incoming message.
    fn handle_message(&mut self, msg: RaftMessage);

    /// Returns the type of the role as an enum.
    fn role_type(&self) -> RoleType;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleType {
    Leader,
    Follower,
    Candidate,
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// Represents a message in the Raft protocol.
pub enum RaftMessage {
    AppendEntries,
    RequestVote,
}