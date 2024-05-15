use std::{collections::HashMap, sync::{Arc, Mutex, PoisonError}};

use pheidippides_messenger::{authorization, User, Message, MessageId, UserId};
use pheidippides_messenger::db::*;

struct MessageRecord {
    id: MessageId,
    from: UserId,
    to: UserId,
    message: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl MessageRecord {
    fn new(from: UserId, to: UserId, message: &str) -> Self {
        let id = uuid::Uuid::new_v4();
        let message = message.into();
        let timestamp = chrono::Utc::now();
        MessageRecord { id , from, to, message, timestamp }
    }
}

#[derive(Debug)]
pub enum Error {
    ThreadPoisonError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThreadPoisonError => write!(f, "Thread poisoning error"),
        }
    }
}

impl std::error::Error for Error {}

impl<T> From<PoisonError<T>> for Error {
    fn from(_value: PoisonError<T>) -> Self {
        Self::ThreadPoisonError
    }
}

struct AuthRecord {
    user_id: UserId,
    phc_string: password_hash::PasswordHashString,
}

#[derive(Clone)]
pub struct Db {
    users: Arc<Mutex<Vec<(UserId, String)>>>,
    messages: Arc<Mutex<Vec<MessageRecord>>>,
    auth: Arc<Mutex<Vec<AuthRecord>>>,
}

impl Db {
    pub async fn new() -> Self {
        let mut users_vec = vec![
            (uuid::Uuid::new_v4(), "User1".into()),
            (uuid::Uuid::new_v4(), "User2".into()),
            (uuid::Uuid::new_v4(), "User3".into()),
            (uuid::Uuid::new_v4(), "Пользователь1".into()),
        ];

        for i in 5..100 {
            users_vec.push((uuid::Uuid::new_v4(), format!("User {i}")))
        };

        let mut username_id_map = HashMap::new();
        for (id, username) in users_vec.iter() {
            username_id_map.insert(username.clone(), id.clone());
        }

        let messages_vec = vec![
            ("User1", "User1", "Hello myself 1"),
            ("User1", "User1", "Hello myself 2"),
            ("User2", "User1", "Hello 1"),
            ("User1", "User2", "Hello 2"),
            ("User2", "User1", "Hello 3"),
            ("User1", "User2", "Hello 4 😊"),
            ("User1", "User3", "Hello 5"),
            ("User3", "User1", "Hello 6"),
        ];

        let mut messages_vec = {
            let mut res = vec![];
            for (from, to, msg) in messages_vec {
                res.push(MessageRecord::new(username_id_map[from], username_id_map[to], msg));
            }
            res
        };

        for i in 0..100 {
            // messages_vec.push(MessageRecord { from: "4".into(), to: "1".into(), message: format!("Привет! ({i})") })
            messages_vec.push(MessageRecord::new(username_id_map["Пользователь1"], username_id_map["User1"], &format!("Привет! ({i})")));
        };

        for (user_id, _) in users_vec.iter().skip(4) {
            // messages_vec.push(MessageRecord { from: user_id.clone(), to: "1".into(), message: "Привет".into() })
            messages_vec.push(MessageRecord::new(*user_id, username_id_map["User1"], "Привет"));
        };

        let users = Arc::new(Mutex::new(users_vec));
        let messages = Arc::new(Mutex::new(messages_vec));
        let auth = Arc::new(Mutex::new(vec![]));
        
        let res = Db { users, messages, auth };

        let credentials = [
            (username_id_map["User1"], "User1"),
            (username_id_map["User2"], "User2"),
        ];

        for (user_id, password) in credentials {
            authorization::create_user(&user_id.to_owned(), password.to_owned(), &res).await.expect("Unable to create authentication while making mock db");
        };
        res
    }
}

impl DataAccess for Db {
    type Error = Error;

    async fn fetch_users(&self) -> Result<Vec<(UserId, String)>, Error> {
        Ok(self.users.lock()?.iter().map(|value| value.clone()).collect())
    }

    async fn create_user(&self, username: &str) -> Result<Option<UserId>, Self::Error> {
        let mut table_locked = self.users.lock()?;

        if table_locked.iter().filter(|record| record.1.to_lowercase() == username.to_lowercase()).next().is_some() {
            return Ok(None)
        };

        let user_id = uuid::Uuid::new_v4();

        table_locked.push((user_id.clone(), username.to_owned()));
        Ok(Some(user_id))
    }

    async fn find_users_chats(&self, user_id: &UserId) -> Result<Vec<User>, Error> {
        let users = self.fetch_users().await?;
        let users = {
            let mut res = HashMap::new();
            for (user_id, username) in users {
                res.insert(user_id, username);
            };
            res
        };
        let res = self.messages.lock()?.iter().rev().filter_map(|msg_record| {
            if &msg_record.from == user_id {
                Some(User::new::<Db>(msg_record.to.clone(), users.get(&msg_record.to).unwrap_or(&"<unknown user id>".to_owned()).clone()))
            } else if &msg_record.to == user_id {
                Some(User::new::<Db>(msg_record.from.clone(), users.get(&msg_record.from).unwrap_or(&"<unknown user id>".to_owned()).clone()))
            } else {
                None
            }
        }).fold(Vec::new(), |mut state, chat_info| {
            if !state.contains(&chat_info) {
                state.push(chat_info)
            }
            state
        });
        Ok(res)
    }

    async fn fetch_last_messages_in_chat(&self, user_id_1: &UserId, user_id_2: &UserId, starting_point: Option<MessageId>) -> Result<Vec<Message>, Error> {
        let res = self.messages.lock()?.iter()
            .rev()
            .skip_while(|msg_record| match &starting_point {
                Some(starting_id) => msg_record.id != *starting_id,
                None => false
            })
            .skip(if starting_point.is_some() {1} else {0})
            .filter_map(|msg_record| {
                if (&msg_record.from == user_id_1 && &msg_record.to == user_id_2)
                    || (&msg_record.from == user_id_2 && &msg_record.to == user_id_1) {
                    Some(Message { id: msg_record.id, from: msg_record.from, to: msg_record.to, message: msg_record.message.to_owned(), timestamp: msg_record.timestamp })
                } else {
                    None
                }
            })
            .take(MESSAGE_LOAD_BUF_SIZE as usize)
            .collect();

        Ok(res)
    }

    async fn fetch_users_messages_since(&self, user_id: &UserId, starting_point: &MessageId) -> Result<Vec<Message>, Self::Error> {
        let res = self.messages.lock()?
            .iter()
            .skip_while(|message_record| message_record.id != *starting_point)
            .skip(1)
            .filter_map(|message_record| {
                let id = message_record.id;
                let from = message_record.from;
                let to = message_record.to;
                let message = message_record.message.clone();
                let timestamp = message_record.timestamp;
                if from == *user_id || to == *user_id {
                    Some(Message { id, from, to, message, timestamp })
                } else {
                    None
                }
            })
            .collect();
        Ok(res)
    }

    async fn create_message(&self, message: &Message) -> Result<(), Error> {
        let mut messages_lock = self.messages.lock()?;
        let new_message = MessageRecord {
            id: message.id,
            from: message.from,
            to: message.to,
            message: message.message.to_owned(),
            timestamp: message.timestamp,
        };
        messages_lock.push(new_message);
        Ok(())
    }

    async fn fetch_authentication(&self, user_id: &UserId) -> Result<Option<AuthenticationInfo>, Error> {
        let res = self.auth.lock()?
            .iter()
            .filter_map(|record|
                if record.user_id == *user_id {
                    Some(AuthenticationInfo::from(record.phc_string.clone()))
                } else {
                    None
                })
            .next();
        Ok(res)
    }

    async fn update_authentication(&self, user_id: &UserId, auth_info: AuthenticationInfo) -> Result<Option<AuthenticationInfo>, Self::Error> {
        let mut table_locked = self.auth.lock()?;
        for record in table_locked.iter_mut() {
            if record.user_id == *user_id {
                let old_auth = record.phc_string.clone();
                record.phc_string = auth_info.phc_string().clone();
                return Ok(Some(old_auth.into()))
            };
        };
        table_locked.push(AuthRecord{ user_id: user_id.clone(), phc_string: auth_info.phc_string().clone() });
        Ok(None)
    }
}