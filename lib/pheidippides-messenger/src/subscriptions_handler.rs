use std::time::Duration;
use std::sync::{Arc, RwLock};
use std::collections::{HashMap, HashSet};
use tokio::sync::broadcast::Sender;
use anyhow::{bail, Context};
use pheidippides_utils::async_utils;
use tokio::sync::mpsc;
use pheidippides_utils::utils::log_internal_error;
use crate::data_access::DataAccess;
use crate::{Message, MessageId, UserId};

const SUBSCRIPTIONS_CLEANUP_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct SubscriptionsHandler<D: DataAccess> {
    data_access: D,
    new_messages: Arc<RwLock<HashMap<UserId, Sender<Message>>>>,
}

impl<D: DataAccess> SubscriptionsHandler<D> {
    pub fn new(data_access: D) -> Self {
        let new_messages_subscriptions: Arc<RwLock<HashMap<UserId, Sender<Message>>>>  = Arc::new(RwLock::new(HashMap::new()));

        Self::spawn_cleanup_job(new_messages_subscriptions.clone());

        SubscriptionsHandler { data_access, new_messages: new_messages_subscriptions }
    }

    pub async fn subscribe_new_messages(&self, user_id: UserId, starting_point: Option<MessageId>) -> anyhow::Result<mpsc::UnboundedReceiver<Message>> {
        let subscription = {
            // let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            let mut subscriptions_lock = match self.new_messages.write() {
                Ok(res) => res,
                Err(e) => bail!("Could not lock new_messages_subscriptions for write: {e}"),
            };

            subscriptions_lock.entry(user_id).or_insert(tokio::sync::broadcast::channel(100).0).subscribe()
        };

        match starting_point {
            None => {
                // no extra channel needed, simply convert broadcast to an unbounded channel
                Ok(async_utils::pipe_broadcast(subscription, |v| Some(v)))
            },
            Some(starting_point) => {
                let previous_messages = self.data_access.fetch_users_messages_since(&user_id, &starting_point).await?;
                let (sender, receiver) = mpsc::unbounded_channel();

                let mut sent_messages = HashSet::new();
                for message in previous_messages {
                    sent_messages.insert(message.id);
                    sender.send(message)?; // Receiver can't be dropped at this point, if .send() returns an error, propagate it back for debugging
                };

                let subscription_filtered = async_utils::pipe_broadcast(subscription, move |message| {
                    if sent_messages.contains(&message.id) {None} else {Some(message)}
                });

                async_utils::redirect_unbounded_channel(subscription_filtered, sender);
                Ok(receiver)
            },
        }
    }

    pub fn handle_new_message(&self, message: &Message) -> anyhow::Result<()> {
        let subscriptions_read = match self.new_messages.read() {
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

    fn spawn_cleanup_job(new_messages_subscriptions: Arc<RwLock<HashMap<UserId, Sender<Message>>>>) {
        // Periodically removes unused subscriptions
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(SUBSCRIPTIONS_CLEANUP_INTERVAL).await;
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

    fn send_event_to_subscribers<T: Clone>(sender: &Sender<T>, event: &T) -> anyhow::Result<()> {
        match sender.send(event.clone()) {
            Ok(_) => Ok(()),
            Err(e) => bail!("{e}")
        }
    }
}
