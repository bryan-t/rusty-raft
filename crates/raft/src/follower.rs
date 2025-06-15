mod Role;
use crate::Role::{self, RoleType};
use crate::RaftMessage;

pub struct Follower {
}
impl Role for Follower {

    fn handle_message(&mut self, msg: RaftMessage) {
        match msg {
            RaftMessage::AppendEntries => {
                // Handle AppendEntries logic
            }
            RaftMessage::RequestVote => {
                // Handle RequestVote logic
            }
        }
    }

    fn role_type(&self) -> RoleType {
        RoleType::Follower
    }
}