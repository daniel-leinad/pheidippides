pub mod mock;
pub mod pg;

use std::cmp::PartialEq;
use std::fmt::Display;
use std::hash::Hash;
use std::future::Future;
use std::str::FromStr;
use thiserror::Error;

use serde::Serialize;

use uuid::Uuid;

pub type MessageId = Uuid;
pub type UserId = Uuid;

// written as a macro to use Self::Error
macro_rules! async_result {
    ($t:ty) => {
        impl Future<Output = Result<$t, Self::Error>> + Send
    };
}

pub trait DbAccess: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    // fn users(&self) -> impl Future<Output = Result<Vec<(UserId, String)>, Self::Error>> + Send;
    fn users(&self) -> async_result!(Vec<(UserId, String)>);
    fn chats(&self, user_id: &UserId) -> async_result!(Vec<ChatInfo>);
    fn last_messages(&self, this: &UserId, other: &UserId, starting_point: Option<MessageId>)-> async_result!(Vec<Message>);
    fn create_message(&self, msg: String, from: &UserId, to: &UserId) -> async_result!(());
    fn authentication(&self, user_id: &UserId) -> async_result!(Option<AuthenticationInfo>);
    fn update_authentication(&self, user_id: &UserId, auth_info: AuthenticationInfo) -> async_result!(Option<AuthenticationInfo>);
    fn create_user(&self, username: &str) -> async_result!(Option<UserId>);
    
    fn username(&self, user_id: &UserId) -> async_result!(Option<String>) {
        async move {
            let users = self.users().await?;
            let res = users  
                .into_iter()
                .filter_map(|(id, username)| {if &id == user_id {Some(username)} else {None}})
                .next();
            Ok(res)
        }
    }

    fn user_id(&self, requested_username: &String) -> async_result!(Option<UserId>) {
        async move {
            let res = self
                .users().await?
                .into_iter()
                .filter_map(|(id, username)| {if &username == requested_username {Some(id)} else {None}})
                .next();
            Ok(res)
        }
    }

    fn find_chats(&self, query: &str) -> async_result!(Vec<ChatInfo>) {
        async {
            let query = query.to_lowercase();
            let res = self.users().await?.into_iter().filter_map(|(user_id, username)| {
                if username.to_lowercase().contains(&query) {
                    Some(ChatInfo::new::<Self>(user_id, username))
                } else {
                    None
                }
            }).collect();
            Ok(res)
        }
    }
}

#[derive(PartialEq, Hash)]
pub struct ChatInfo {
    pub username: String,
    pub id: UserId,
}

impl ChatInfo {
    fn new<T: DbAccess>(id: UserId, username: String) -> Self
    {
        ChatInfo {username, id}
    }
}

#[derive(Serialize)]
pub struct Message {
    #[serde(serialize_with = "crate::utils::serialize_uuid")]
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

pub struct AuthenticationInfo {
    phc_string: password_hash::PasswordHashString,
}

impl AuthenticationInfo {
    pub fn phc_string(&self) -> &password_hash::PasswordHashString {
        &self.phc_string
    }
}

impl<'a> From<password_hash::PasswordHash<'a>> for AuthenticationInfo {
    fn from(value: password_hash::PasswordHash<'a>) -> Self {
        AuthenticationInfo { phc_string: value.into() }
    }
}

impl From<password_hash::PasswordHashString> for AuthenticationInfo {
    fn from(value: password_hash::PasswordHashString) -> Self {
        AuthenticationInfo { phc_string: value }
    }
}

#[derive(Error, Debug)]
pub enum AuthenticationInfoParsingError {
    #[error("Incorrect phc string: {0}")]
    IncorrectPHCString(String),
}

impl FromStr for AuthenticationInfo {
    type Err = AuthenticationInfoParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse() {
            Ok(phc_string) => Ok(AuthenticationInfo{phc_string}),
            Err(_) => Err(AuthenticationInfoParsingError::IncorrectPHCString(s.to_owned()))
        }
    }
}