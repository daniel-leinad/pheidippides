pub mod mock;

use std::cmp::PartialEq;
use std::hash::Hash;

pub trait DbAccess: Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    fn users(&self) -> Result<Vec<(UserId, String)>, Self::Error>;
    fn chats(&self, user_id: &UserId) -> Result<Vec<ChatInfo>, Self::Error>;
    fn messages(&self, this: &UserId, other: &UserId)-> Result<Vec<Message>, Self::Error>;
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
}

pub type UserId = String;

#[derive(PartialEq, Hash)]
pub struct ChatInfo {
    pub username: String,
    pub id: UserId,
}

impl ChatInfo {
    fn new(username: String, id: UserId) -> Self {
        ChatInfo {username, id}
    }
}

pub enum Message {
    In(String),
    Out(String),
}