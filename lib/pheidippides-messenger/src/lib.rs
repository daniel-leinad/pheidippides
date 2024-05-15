use uuid::Uuid;
use serde::Serialize;
use chrono::DateTime;
use crate::db::DataAccess;

pub mod app;
pub mod db;
pub mod authorization;

pub type MessageId = Uuid;
pub type UserId = Uuid;

#[derive(PartialEq, Hash)]
pub struct User {
    pub username: String,
    pub id: UserId,
}

impl User {
    pub fn new<T: DataAccess>(id: UserId, username: String) -> Self
    {
        User {username, id}
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
