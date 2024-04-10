use crate::db::{ChatInfo, DbAccess, MessageId, UserId, Message};
use crate::utils::log_internal_error;
use anyhow::{Context, Result, bail};
use crate::authorization;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Clone)]
pub struct App<D: DbAccess> {
    db_access: D,
    new_messages_subscriptions: Arc<RwLock<HashMap<UserId, Vec<UnboundedSender<Message>>>>>,
}

pub enum UserCreationError {
    UsernameTaken,
}

impl<D: DbAccess> App<D> {
    pub fn new(db_access: D) -> Self {
        let new_messages_subscriptions = Arc::new(RwLock::new(HashMap::new()));
        App { db_access, new_messages_subscriptions }
    }

    pub async fn create_user(&self, login: &str, password: String) -> Result<Option<UserId>> {
        let user_id = match self.db_access
            .create_user(login).await
            .with_context(|| format!("Couldn't create user {}", login))? {
            Some(user_id) => user_id,
            None => return Ok(None),
        };

        authorization::create_user(&user_id, password, &self.db_access).await.with_context(
            || format!("Authoriazation error: couldn't create user {}", login))?;
        
        Ok(Some(user_id))
    }

    pub async fn verify_user(&self, username: &str, password: String) -> Result<Option<UserId>> {
        let user_id = match self.user_id(username).await? {
            Some(user_id) => user_id,
            None => return Ok(None),
        };

        let res = authorization
            ::verify_user(&user_id, password, &self.db_access).await
            .with_context(|| format!("Authorization error: couldn't verify user {}", &user_id))?;

        if res {
            Ok(Some(user_id))
        } else {
            Ok(None)
        }
    }

    pub async fn fetch_users_chats(&self, user_id: &UserId) -> Result<Vec<ChatInfo>> {
        
        let chats = self.db_access
                .chats(&user_id).await
                .with_context(|| format!("Couldn't fetch chats for user {user_id}"))?;
        
        Ok(chats)
    }

    pub async fn username(&self, user_id: &UserId) -> Result<Option<String>> {
        self.db_access.username(user_id).await.with_context(|| format!("Couldn't fetch username for id {user_id}"))
    }

    pub async fn send_message(&self, message_text: &str, from: &UserId, to: &UserId) -> Result<MessageId> {
        let message_id = self.db_access
            .create_message(message_text, from, to).await
            .with_context(|| format!("Couldn't create message from {from} to {to}"))?;

        match self.new_messages_subscriptions.read() {
            Ok(subscriptions_read) => {
                let mut informational_hash_map = HashMap::new();
                for (key, value) in subscriptions_read.iter() {
                    informational_hash_map.insert(key, value.len());
                }
                eprintln!("{informational_hash_map:?}");

                let message = Message {
                    id: message_id,
                    from: from.to_owned(),
                    to: to.to_owned(),
                    message: message_text.to_owned(),
                };

                if let Some(subscriptions) = subscriptions_read.get(from) {
                    for subscription in subscriptions {                    
                        subscription.send(message.clone());
                    }
                };

                if from != to {
                    if let Some(subscriptions) = subscriptions_read.get(to) {
                        for subscription in subscriptions {                    
                            subscription.send(message.clone());
                        }
                    };
                }
            },
            Err(e) => {
                log_internal_error(e);
            },
        }

        Ok(message_id)
    }

    pub async fn find_chats(&self, query: &str) -> Result<Vec<ChatInfo>> {
        let chats = self.db_access
            .find_chats(query).await
            .with_context(|| format!("Could't process chats search request with query: {query}"))?;
        Ok(chats)
    }

    pub async fn fetch_last_messages(&self, current_user: &UserId, other_user: &UserId, starting_point: Option<MessageId>) -> Result<Vec<Message>> {
        self.db_access.last_messages(current_user, other_user, starting_point).await
            .with_context(|| format!("Could not fetch last messages.\
                current_user: {current_user}, other_user: {other_user}, starting_point: {starting_point:?}"))
    }

    pub async fn subscribe_new_messages(&self, user_id: UserId, starting_point: Option<MessageId>) -> Result<UnboundedReceiver<Message>> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut subscriptions_lock = match self.new_messages_subscriptions.write() {
            Ok(res) => res,
            Err(e) => bail!("Could not lock new_messages_subscriptions for write: {e}"),
        };
            
        subscriptions_lock.entry(user_id).or_insert(vec![]).push(sender);
        Ok(receiver)
    }

    async fn user_id(&self, username: &str) -> Result<Option<UserId>> {
        self.db_access.user_id(username).await.with_context(|| format!("Couldn't fetch user_id for username {username}"))
    }
}