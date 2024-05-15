use uuid::Uuid;
use chrono::DateTime;
use crate::data_access::DataAccess;

pub mod messenger;
pub mod data_access;
pub mod authorization;
mod subscriptions_handler;

pub type MessageId = Uuid;
pub type UserId = Uuid;

#[derive(PartialEq, Hash, Debug)]
pub struct User {
    pub username: String,
    pub id: UserId,
}

impl User {
    pub fn new<T: DataAccess>(id: UserId, username: String) -> Self { User {username, id} }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Message {
    pub id: MessageId,
    pub from: UserId,
    pub to: UserId,
    pub message: String,
    pub timestamp: DateTime<chrono::Utc>,
}