pub mod mock;

use std::cmp::PartialEq;
use std::hash::Hash;

use serde::Serialize;

pub type UserId = String;
pub type MessageId = String;

pub trait DbAccess: 'static + Send + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    fn users(&self) -> Result<Vec<(UserId, String)>, Self::Error>;
    fn chats(&self, user_id: &UserId) -> Result<Vec<ChatInfo>, Self::Error>;
    fn last_messages(&self, this: &UserId, other: &UserId, starting_point: Option<MessageId>)-> Result<Vec<Message>, Self::Error>;
    fn create_message(&self, msg: String, from: &UserId, to: &UserId) -> Result<(), Self::Error>;
    
    fn username(&self, user_id: &UserId) -> Result<Option<String>, Self::Error> {
        let res = self
            .users()?
            .into_iter()
            .filter_map(|(id, username)| {if &id == user_id {Some(username)} else {None}})
            .next();
        Ok(res)
    }

    fn user_id(&self, requested_username: &String) -> Result<Option<UserId>, Self::Error> {
        let res = self
            .users()?
            .into_iter()
            .filter_map(|(id, username)| {if &username == requested_username {Some(id)} else {None}})
            .next();
        Ok(res)
    }

    fn find_chats(&self, query: &str) -> Result<Vec<ChatInfo>, Self::Error> {
        let query = query.to_lowercase();
        let res = self.users()?.into_iter().filter_map(|(user_id, username)| {
            if username.to_lowercase().contains(&query) {
                Some(ChatInfo::new(user_id, username))
            } else {
                None
            }
        }).collect();
        Ok(res)
    }
}

#[derive(PartialEq, Hash)]
pub struct ChatInfo {
    pub username: String,
    pub id: UserId,
}

impl ChatInfo {
    fn new(id: UserId, username: String) -> Self {
        ChatInfo {username, id}
    }
}

#[derive(Serialize)]
pub struct Message {
    pub id: MessageId,
    #[serde(rename(serialize = "type"))]
    pub message_type: MessageType,
    pub message: String,
}

#[derive(Serialize)]
pub enum MessageType {
    In,
    Out,
}