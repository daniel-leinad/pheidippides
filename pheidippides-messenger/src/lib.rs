use uuid::Uuid;
use serde::Serialize;
use chrono::DateTime;
use crate::db::DbAccess;

pub mod app;
pub mod db;
pub mod authorization;

pub type MessageId = Uuid;
pub type UserId = Uuid;

#[derive(PartialEq, Hash)]
pub struct Chat {
    pub username: String,
    pub id: UserId,
}

impl Chat {
    pub fn new<T: DbAccess>(id: UserId, username: String) -> Self
    {
        Chat {username, id}
    }
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Message {
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_uuid")]
    pub id: MessageId,
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_uuid")]
    pub from: UserId,
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_uuid")]
    pub to: UserId,
    pub message: String,
    #[serde(serialize_with = "pheidippides_utils::serde::serialize_datetime")]
    pub timestamp: DateTime<chrono::Utc>,
}
