use crate::data_access::DataAccess;
use chrono::DateTime;
use uuid::Uuid;

pub mod authorization;
pub mod data_access;
pub mod messenger;
mod subscriptions_handler;

pub type MessageId = Uuid;
pub type UserId = Uuid;

#[derive(PartialEq, Hash, Debug)]
pub struct User {
    pub username: String,
    pub id: UserId,
}

impl User {
    pub fn new<T: DataAccess>(id: UserId, username: String) -> Self {
        User { username, id }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Message {
    pub id: MessageId,
    pub from: UserId,
    pub to: UserId,
    pub message: String,
    pub timestamp: DateTime<chrono::Utc>,
}
