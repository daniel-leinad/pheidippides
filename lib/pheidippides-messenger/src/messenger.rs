use anyhow::{Context, Result};
use tokio::sync::mpsc;
use pheidippides_utils::utils::log_internal_error;

use crate::data_access::DataAccess;
use crate::{Message, MessageId, User, UserId};
use crate::subscriptions_handler::SubscriptionsHandler;
use crate::authorization::{AuthService, AuthStorage};

#[derive(Clone)]
pub struct Messenger<D, T> {
    data_access: D,
    authorization_service: AuthService<T>,
    subscriptions_handler: SubscriptionsHandler<D>,
}

pub enum UserCreationError {
    UsernameTaken,
}

impl<D: DataAccess, A> Messenger<D, A> {
    pub fn new(data_access: D, auth_storage: A) -> Self {
        let subscriptions_handler = SubscriptionsHandler::new(data_access.clone());
        let authorization_service = AuthService::new(auth_storage);
        Messenger { data_access, authorization_service, subscriptions_handler }
    }

    pub async fn fetch_user(&self, user_id: &UserId) -> Result<Option<User>> {
        let user = self.data_access
            .fetch_user(user_id).await
            .with_context(|| format!("Couldn't fetch user with id {user_id}"))?;
        Ok(user)
    }

    async fn find_user_by_username(&self, username: &str) -> Result<Option<UserId>> {
        self.data_access.find_user_by_username(username).await.with_context(|| format!("Couldn't fetch user_id for username {username}"))
    }

    pub async fn fetch_username(&self, user_id: &UserId) -> Result<Option<String>> {
        // TODO delete this?
        let user = self.fetch_user(user_id).await?;
        Ok(user.map(|user| user.username))
    }

    pub async fn fetch_users_chats(&self, user_id: &UserId) -> Result<Vec<User>> {

        let chats = self.data_access
            .find_users_chats(&user_id).await
            .with_context(|| format!("Couldn't fetch chats for user {user_id}"))?;

        Ok(chats)
    }

    pub async fn find_users_by_substring(&self, substring: &str) -> Result<Vec<User>> {
        let users = self.data_access
            .find_users_by_substring(substring).await
            .with_context(|| format!("Could't process users search request by substring: {substring}"))?;
        Ok(users)
    }

    pub async fn send_message(&self, message_text: String, from: UserId, to: UserId) -> Result<MessageId> {
        let message = Message {
            id: uuid::Uuid::new_v4(),
            from,
            to,
            message: message_text,
            timestamp: chrono::Utc::now(),
        };

        self.data_access
            .create_message(&message).await
            .with_context(|| format!("Couldn't create message from {from} to {to}"))?;

        if let Err(e) = self.subscriptions_handler.handle_new_message(&message) {
            log_internal_error(e);
        };

        Ok(message.id)
    }

    pub async fn fetch_last_messages(&self, current_user: &UserId, other_user: &UserId, starting_point: Option<MessageId>) -> Result<Vec<Message>> {
        self.data_access.fetch_last_messages_in_chat(current_user, other_user, starting_point).await
            .with_context(|| format!("Could not fetch last messages.\
                current_user: {current_user}, other_user: {other_user}, starting_point: {starting_point:?}"))
    }

    pub async fn subscribe_to_new_messages(&self, user_id: UserId, starting_point: Option<MessageId>) -> Result<mpsc::UnboundedReceiver<Message>> {
        self.subscriptions_handler.subscribe_new_messages(user_id, starting_point).await
    }
}

impl<D: DataAccess, A: AuthStorage> Messenger<D, A> {
    pub async fn verify_user(&self, username: &str, password: String) -> Result<Option<UserId>> {
        let user_id = match self.find_user_by_username(username).await? {
            Some(user_id) => user_id,
            None => return Ok(None),
        };

        let res = self.authorization_service
            .verify_user(&user_id, password).await
            .with_context(|| format!("Authorization error: couldn't verify user {}", &user_id))?;

        if res {
            Ok(Some(user_id))
        } else {
            Ok(None)
        }
    }

    pub async fn create_user(&self, login: &str, password: String) -> Result<Option<UserId>> {
        let user_id = match self.data_access
            .create_user(login).await
            .with_context(|| format!("Couldn't create user {}", login))? {
            Some(user_id) => user_id,
            None => return Ok(None),
        };

        self.authorization_service.create_user(&user_id, password).await.with_context(
            || format!("Authorization error: couldn't create user {}", login))?;

        Ok(Some(user_id))
    }
}