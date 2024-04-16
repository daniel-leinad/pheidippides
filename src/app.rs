use crate::db::{ChatInfo, DbAccess, MessageId, UserId, Message};
use crate::utils::log_internal_error;
use anyhow::{Context, Result, bail};
use crate::authorization;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock, Mutex};
use std::time::Duration;
// use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::broadcast::{Receiver, Sender};

const SUBSCRIPTION_GARBAGE_COLLECTION_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct App<D: DbAccess> {
    db_access: D,
    new_messages_subscriptions: Arc<RwLock<HashMap<UserId, Sender<Message>>>>,
}

pub enum UserCreationError {
    UsernameTaken,
}

impl<D: DbAccess> App<D> {
    pub fn new(db_access: D) -> Self {
        let new_messages_subscriptions: Arc<RwLock<HashMap<UserId, Sender<Message>>>>  = Arc::new(RwLock::new(HashMap::new()));

        Self::spawn_subscription_garbage_collector(new_messages_subscriptions.clone());
        
        App { db_access, new_messages_subscriptions }
    }

    fn spawn_subscription_garbage_collector(new_messages_subscriptions: Arc<RwLock<HashMap<UserId, Sender<Message>>>>) {
        // Periodically removes unused subscriptions
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(SUBSCRIPTION_GARBAGE_COLLECTION_INTERVAL).await;
                match new_messages_subscriptions.write() {
                    Ok(mut write_lock) => {
                        write_lock.retain(|_, sender| sender.receiver_count() > 0);
                        write_lock.shrink_to_fit();
                    },
                    Err(e) => log_internal_error(e),
                }
            }
        });
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

    pub async fn send_message(&self, message_text: String, from: UserId, to: UserId) -> Result<MessageId> {
        let message = Message {
            id: uuid::Uuid::new_v4(),
            from: from,
            to: to,
            message: message_text,
            timestamp: chrono::Utc::now(),
        };

        self.db_access
            .create_message(&message).await
            .with_context(|| format!("Couldn't create message from {from} to {to}"))?;

        if let Err(e) = self.handle_message_subscription(&message) {
            log_internal_error(e);
        };

        Ok(message.id)
    }

    fn handle_message_subscription(&self, message: &Message) -> Result<()> {
        let subscriptions_read = match self.new_messages_subscriptions.read() {
            Ok(read_lock) => read_lock,
            Err(e) => bail!("Could not lock new_messages_subscriptions for read: {e}"),
        };

        if let Some(sender) = subscriptions_read.get(&message.from) {
            Self::send_event_to_subscribers(sender, message)
                    .with_context(|| format!("Couldn't send subscription events for {}", &message.from))?;
        };

        if message.from != message.to {
            if let Some(sender) = subscriptions_read.get(&message.to) {
                Self::send_event_to_subscribers(sender, message)
                    .with_context(|| format!("Couldn't send subscription events for {}", &message.to))?;
            };
        };

        Ok(())
    }

    fn send_event_to_subscribers<T: Clone>(sender: &Sender<T>, event: &T) -> Result<()> {
        match sender.send(event.clone()) {
            Ok(_) => Ok(()),
            Err(e) => bail!("{e}")
        }
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

    pub async fn subscribe_new_messages(&self, user_id: UserId, starting_point: Option<MessageId>) -> Result<Receiver<Message>> {
        // let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut subscriptions_lock = match self.new_messages_subscriptions.write() {
            Ok(res) => res,
            Err(e) => bail!("Could not lock new_messages_subscriptions for write: {e}"),
        };

        let receiver = subscriptions_lock.entry(user_id).or_insert(tokio::sync::broadcast::channel(100).0).subscribe();

        match starting_point {
            None => Ok(receiver), // no extra channel needed, simply receive new messages
            Some(starting_point) => { todo!() }
        }
    }

    async fn user_id(&self, username: &str) -> Result<Option<UserId>> {
        self.db_access.user_id(username).await.with_context(|| format!("Couldn't fetch user_id for username {username}"))
    }
}