pub mod mock;
pub mod pg;

use std::cmp::PartialEq;
use std::hash::Hash;
use std::future::Future;
use std::str::FromStr;
use chrono::DateTime;
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

// TODO rewrite trait to accept references as much as possible
pub trait DbAccess: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    // fn users(&self) -> impl Future<Output = Result<Vec<(UserId, String)>, Self::Error>> + Send;
    fn users(&self) -> async_result!(Vec<(UserId, String)>);
    fn chats(&self, user_id: &UserId) -> async_result!(Vec<ChatInfo>);
    fn last_messages(&self, user_id_1: &UserId, user_id_2: &UserId, starting_point: Option<MessageId>)-> async_result!(Vec<Message>);
    fn users_messages_since(&self, user_id: &UserId, starting_point: &MessageId) -> async_result!(Vec<Message>);
    fn create_message(&self, message: &Message) -> async_result!(());
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

    fn user_id(&self, requested_username: &str) -> async_result!(Option<UserId>) {
        async move {
            let res = self
                .users().await?
                .into_iter()
                .filter_map(|(id, username)| {if username.to_lowercase() == requested_username.to_lowercase() {Some(id)} else {None}})
                .next();
            Ok(res)
        }
    }

    fn find_chats(&self, search_query: &str) -> async_result!(Vec<ChatInfo>) {
        async {
            let search_query = search_query.to_lowercase();
            let res = self.users().await?.into_iter().filter_map(|(user_id, username)| {
                if username.to_lowercase().contains(&search_query) {
                    Some(ChatInfo::new::<Self>(user_id, username))
                } else {
                    None
                }
            }).collect();
            Ok(res)
        }
    }

    fn chat_info(&self, user_id: &UserId) -> async_result!(Option<ChatInfo>) {
        async move {
            let chat_info = self.users().await?
                .into_iter().filter_map(|(id, username)| {
                    if &id == user_id {
                        let id = id.to_owned();
                        let username = username.to_owned();
                        Some(ChatInfo { username, id })
                    } else {
                        None
                    }
                })
                .next();
            Ok(chat_info)
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

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct Message {
    #[serde(serialize_with = "crate::utils::serialize_uuid")]
    pub id: MessageId,
    #[serde(serialize_with = "crate::utils::serialize_uuid")]
    pub from: UserId,
    #[serde(serialize_with = "crate::utils::serialize_uuid")]
    pub to: UserId,
    pub message: String,
    #[serde(serialize_with = "crate::utils::serialize_datetime")]
    pub timestamp: DateTime<chrono::Utc>,
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

#[cfg(test)]
mod tests {
    use std::{future::Future, time::Duration};

    use super::{DbAccess, Message};
    use anyhow::{Result, Context, ensure};
    use chrono::DateTime;
    use uuid::{timestamp, Uuid};

    #[macro_export]
    macro_rules! db_access_tests {
        ($tester:ident) => {
            $tester!{it_creates_user}
            $tester!{doesnt_fetch_nonexistent_users}
            $tester!{it_creates_message}
            $tester!{fetches_last_messages}
            $tester!{fetches_users_messages_since}
        };
    }

    pub async fn it_creates_user(db_access: &impl DbAccess) {
        let username = "TestUser1";
        let user_id = db_access.create_user(username).await
            .unwrap()
            .unwrap();
        assert_eq!(db_access.username(&user_id).await.unwrap().unwrap(), username);
        assert_eq!(db_access.user_id(username).await.unwrap().unwrap(), user_id);
    }

    pub async fn doesnt_fetch_nonexistent_users(db_access: &impl DbAccess) {
        assert!(db_access.username(&uuid::Uuid::new_v4()).await.unwrap().is_none());
        assert!(db_access.user_id("__NonExistentUserOnlyForTesting").await.unwrap().is_none());
    }

    pub async fn it_creates_message(db_access: &impl DbAccess) {
        let sender_id = db_access.create_user("__Test_Sender").await.unwrap().unwrap();
        let receiver_id = db_access.create_user("__Test_Receiver").await.unwrap().unwrap();
        let message = Message{ 
            id: Uuid::new_v4(), 
            from: sender_id, 
            to: receiver_id, 
            message: "Test message".to_owned(), 
            timestamp: chrono::Utc::now(), 
        };

        db_access.create_message(&message).await.unwrap();
    }

    pub async fn fetches_last_messages(db_access: &impl DbAccess) {
        let user_1 = db_access.create_user("__User_1").await.unwrap().unwrap();
        let user_2 = db_access.create_user("__User_2").await.unwrap().unwrap();
        let user_3 = db_access.create_user("__User_3").await.unwrap().unwrap();

        let messages = [
            (user_1, user_2, "Message 1"),
            (user_2, user_1, "Message 2"),
            (user_1, user_2, "Message 3"), // starting point
            (user_2, user_1, "Message 4"),
            (user_1, user_3, "Message 5"),
            (user_2, user_3, "Message 6"),
            (user_3, user_2, "Message 7"),
            (user_3, user_1, "Message 8"),
        ];

        //TODO assert that message len fits into buffer

        let mut starting_id = Uuid::nil();

        let mut timestamp = chrono::Utc::now();

        for (from, to, msg) in messages {
            timestamp = timestamp + Duration::from_secs(1);
            let id = Uuid::new_v4();
            if msg == "Message 3" {
                starting_id = id;
            };
            db_access.create_message(&Message { 
                id: id, 
                from, 
                to, 
                message: msg.to_owned(), 
                timestamp,
            }).await.unwrap();
        };

        let mut last_messages = db_access
            .last_messages(&user_1, &user_2, None)
            .await
            .unwrap()
            .into_iter();

        assert_eq!(&last_messages.next().unwrap().message, "Message 4");
        assert_eq!(&last_messages.next().unwrap().message, "Message 3");
        assert_eq!(&last_messages.next().unwrap().message, "Message 2");
        assert_eq!(&last_messages.next().unwrap().message, "Message 1");
        assert_eq!(last_messages.next(), None);

        let mut last_messages = db_access
            .last_messages(&user_3, &user_2, None)
            .await
            .unwrap()
            .into_iter();

        assert_eq!(&last_messages.next().unwrap().message, "Message 7");
        assert_eq!(&last_messages.next().unwrap().message, "Message 6");
        assert_eq!(last_messages.next(), None);

        let mut last_messages = db_access
            .last_messages(&user_1, &user_2, Some(starting_id))
            .await
            .unwrap()
            .into_iter();

        assert_eq!(&last_messages.next().unwrap().message, "Message 2");
        assert_eq!(&last_messages.next().unwrap().message, "Message 1");
        assert_eq!(last_messages.next(), None);

    }

    pub async fn fetches_users_messages_since(db_access: &impl DbAccess) {
        let user_1 = db_access.create_user("__User_1").await.unwrap().unwrap();
        let user_2 = db_access.create_user("__User_2").await.unwrap().unwrap();
        let user_3 = db_access.create_user("__User_3").await.unwrap().unwrap();

        let messages = [
            (user_1, user_2, "Message 1"),
            (user_2, user_1, "Message 2"),
            (user_1, user_2, "Message 3"), // starting point
            (user_2, user_1, "Message 4"),
            (user_1, user_3, "Message 5"),
            (user_2, user_3, "Message 6"),
            (user_3, user_2, "Message 7"),
            (user_3, user_1, "Message 8"),
        ];

        let mut starting_id = Uuid::nil();

        let mut timestamp = chrono::Utc::now();

        for (from, to, msg) in messages {
            timestamp = timestamp + Duration::from_secs(1);
            let id = Uuid::new_v4();
            if msg == "Message 3" {
                starting_id = id;
            };
            db_access.create_message(&Message { 
                id: id, 
                from, 
                to, 
                message: msg.to_owned(), 
                timestamp,
            }).await.unwrap();
        };

        let mut users_messages_since = db_access
            .users_messages_since(&user_1, &starting_id)
            .await
            .unwrap()
            .into_iter();
        
        assert_eq!(&users_messages_since.next().unwrap().message, "Message 4");
        assert_eq!(&users_messages_since.next().unwrap().message, "Message 5");
        assert_eq!(&users_messages_since.next().unwrap().message, "Message 8");
        assert_eq!(users_messages_since.next(), None);
    }
}