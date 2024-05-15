use std::future::Future;
use std::str::FromStr;

use thiserror::Error;

use crate::{User, Message, MessageId, UserId};

pub const MESSAGE_LOAD_BUF_SIZE: i32 = 50;

// written as a macro to use Self::Error
macro_rules! async_result {
    ($t:ty) => {
        impl Future<Output = Result<$t, Self::Error>> + Send
    };
}

// TODO rewrite trait to accept references as much as possible
pub trait DataAccess: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    // fn users(&self) -> impl Future<Output = Result<Vec<(UserId, String)>, Self::Error>> + Send;
    fn fetch_users(&self) -> async_result!(Vec<(UserId, String)>);
    fn find_users_chats(&self, user_id: &UserId) -> async_result!(Vec<User>);
    fn fetch_last_messages_in_chat(&self, user_id_1: &UserId, user_id_2: &UserId, starting_point: Option<MessageId>) -> async_result!(Vec<Message>);
    fn fetch_users_messages_since(&self, user_id: &UserId, starting_point: &MessageId) -> async_result!(Vec<Message>);
    fn create_message(&self, message: &Message) -> async_result!(());
    fn fetch_authentication(&self, user_id: &UserId) -> async_result!(Option<AuthenticationInfo>);
    fn update_authentication(&self, user_id: &UserId, auth_info: AuthenticationInfo) -> async_result!(Option<AuthenticationInfo>);
    fn create_user(&self, username: &str) -> async_result!(Option<UserId>);

    fn find_user_by_username(&self, requested_username: &str) -> async_result!(Option<UserId>) {
        async move {
            let res = self
                .fetch_users().await?
                .into_iter()
                .filter_map(|(id, username)| {if username.to_lowercase() == requested_username.to_lowercase() {Some(id)} else {None}})
                .next();
            Ok(res)
        }
    }

    fn find_users(&self, search_query: &str) -> async_result!(Vec<User>) {
        async {
            let search_query = search_query.to_lowercase();
            let res = self.fetch_users().await?.into_iter().filter_map(|(user_id, username)| {
                if username.to_lowercase().contains(&search_query) {
                    Some(User::new::<Self>(user_id, username))
                } else {
                    None
                }
            }).collect();
            Ok(res)
        }
    }

    fn fetch_user(&self, user_id: &UserId) -> async_result!(Option<User>) {
        async move {
            let chat_info = self.fetch_users().await?
                .into_iter().filter_map(|(id, username)| {
                    if &id == user_id {
                        let id = id.to_owned();
                        let username = username.to_owned();
                        Some(User { username, id })
                    } else {
                        None
                    }
                })
                .next();
            Ok(chat_info)
        }
    }
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