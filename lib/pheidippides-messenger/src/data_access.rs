use std::future::Future;
use crate::{Message, MessageId, User, UserId};

pub const MESSAGE_LOAD_BUF_SIZE: i32 = 50;

// written as a macro to use Self::Error
macro_rules! async_result {
    ($t:ty) => {
        impl Future<Output = Result<$t, Self::Error>> + Send
    };
}

pub trait DataAccess: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    fn fetch_users(&self) -> async_result!(Vec<(UserId, String)>);
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

    fn find_users_by_substring(&self, substring: &str) -> async_result!(Vec<User>) {
        async {
            let search_query = substring.to_lowercase();
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
    fn create_user(&self, username: &str) -> async_result!(Option<UserId>);

    fn find_users_chats(&self, user_id: &UserId) -> async_result!(Vec<User>);

    fn fetch_last_messages_in_chat(&self, user_id_1: &UserId, user_id_2: &UserId, starting_point: Option<&MessageId>) -> async_result!(Vec<Message>);
    fn fetch_users_messages_since(&self, user_id: &UserId, starting_point: &MessageId) -> async_result!(Vec<Message>);
    fn create_message(&self, message: &Message) -> async_result!(());
}