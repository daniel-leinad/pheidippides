use crate::authorization;

use super::*;
use std::{collections::HashMap, sync::{Arc, Mutex, PoisonError}};

const MESSAGE_LOAD_BUF_SIZE: usize = 50;

struct MessageRecord {
    id: MessageId,
    from: UserId,
    to: UserId,
    message: String,
}

impl MessageRecord {
    fn new(id: &str, from: &str, to: &str, message: &str) -> Self {
        let id = id.into();
        let from = from.into();
        let to = to.into();
        let message = message.into();
        MessageRecord { id , from, to, message }
    }
}

#[derive(Debug)]
pub enum Error {
    ThreadPoisonError,
    IncorrectMessageId(MessageId),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThreadPoisonError => write!(f, "Thread poisoning error"),
            Self::IncorrectMessageId(msg_id) => write!(f, "Incorrect message id: {msg_id}")
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
    last_user_id: Arc<Mutex<u32>>,
    messages: Arc<Mutex<Vec<MessageRecord>>>,
    auth: Arc<Mutex<Vec<AuthRecord>>>,
}

impl Db {
    pub fn new() -> Self {
        let mut users_vec = vec![
            ("1".into(), "Dan".into()),
            ("2".into(), "Man".into()),
            ("3".into(), "John".into()),
            ("4".into(), "Разговорчивый".into()),
        ];

        for i in 5..100 {
            users_vec.push((format!("{i}"), format!("User {i}")))
        };

        let mut messages_vec = vec![
            MessageRecord::new("1", "1", "1", "Привет!"),
            MessageRecord::new("2", "1", "1", "Ой, я написал самому себе..."),
            MessageRecord::new("3", "2", "1", "Hey"),
            MessageRecord::new("4", "1", "2", "Hey, man.."),
            MessageRecord::new("5", "2", "1", "Actually, I AM Man..."),
            MessageRecord::new("6", "1", "2", "Right... 😂😂😂"),
            MessageRecord::new("7", "1", "3", "Hey, John, like your new song!"),
            MessageRecord::new("8", "3", "1", "Thanks, it's very popular, can you imagine that?"),
        ];

        let mut next_id = 9;

        for i in 0..100 {
            // messages_vec.push(MessageRecord { from: "4".into(), to: "1".into(), message: format!("Привет! ({i})") })
            messages_vec.push(MessageRecord::new(&format!("{next_id}"), "4", "1", &format!("Привет! ({i})")));
            next_id += 1;
        };

        for (user_id, _) in users_vec.iter().skip(4) {
            // messages_vec.push(MessageRecord { from: user_id.clone(), to: "1".into(), message: "Привет".into() })
            messages_vec.push(MessageRecord::new(&format!("{next_id}"), &user_id, "1", "Привет"));
            next_id += 1;
        };

        let users = Arc::new(Mutex::new(users_vec));
        let messages = Arc::new(Mutex::new(messages_vec));
        let auth = Arc::new(Mutex::new(vec![]));
        let last_user_id = Arc::new(Mutex::new(next_id));
        
        let res = Db { users, messages, auth, last_user_id };

        let credentials = [
            ("1", "Dan"),
            ("2", "123"),
        ];

        for (user_id, password) in credentials {
            authorization::create_user(&user_id.to_owned(), password, &res).expect("Unable to create authentication while making mock db");
        };
        res
    }
}

impl DbAccess for Db {
    type Error = Error;
    fn users(&self) -> Result<Vec<(UserId, String)>, Error> {
        Ok(self.users.lock()?.iter().map(|value| value.clone()).collect())
    }

    fn chats(&self, user_id: &UserId) -> Result<Vec<ChatInfo>, Error> {
        let users = self.users()?;
        let users = {
            let mut res = HashMap::new();
            for (user_id, username) in users {
                res.insert(user_id, username);
            };
            res
        };
        let res = self.messages.lock()?.iter().filter_map(|msg_record| {
            if &msg_record.from == user_id {
                Some(ChatInfo::new(msg_record.to.clone(), users.get(&msg_record.to).unwrap_or(&"<unknown user id>".to_owned()).clone()))
            } else if &msg_record.to == user_id {
                Some(ChatInfo::new(msg_record.from.clone(), users.get(&msg_record.from).unwrap_or(&"<unknown user id>".to_owned()).clone()))
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

    fn last_messages(&self, this: &UserId, other: &UserId, starting_point: Option<MessageId>) -> Result<Vec<Message>, Error> {
        let res = self.messages.lock()?.iter()
            .rev()
            .skip_while(|msg_record| match &starting_point {
                Some(starting_id) => msg_record.id != *starting_id,
                None => false    
            })
            .skip(if starting_point.is_some() {1} else {0})
            .filter_map(|msg_record| {
                if &msg_record.from == this && &msg_record.to == other {
                    // Some(Message::Out(msg_record.id.clone(), msg_record.message.clone()))
                    Some(Message { id: msg_record.id.clone(), message_type: MessageType::Out, message: msg_record.message.clone() })
                } else if &msg_record.from == other && &msg_record.to == this {
                    // Some(Message::In(msg_record.id.clone(), msg_record.message.clone()))
                    Some(Message { id: msg_record.id.clone(), message_type: MessageType::In, message: msg_record.message.clone() })
                } else {
                    None
                }
            })
            .take(MESSAGE_LOAD_BUF_SIZE)
            .collect();

        Ok(res)
    }
    
    fn create_message(&self, message: String, from: &UserId, to: &UserId) -> Result<(), Self::Error> {
        let mut messages_lock = self.messages.lock()?;
        let last_id = messages_lock.last().map(|msg_record| msg_record.id.clone()).unwrap_or("-1".to_owned());
        let last_id: i32 = match last_id.parse() {
            Ok(i) => i,
            Err(_) => return Err(Error::IncorrectMessageId(last_id)),
        };
        let new_id: String = format!("{}", last_id + 1);
        messages_lock.push(MessageRecord::new(&new_id, from, to, &message));
        Ok(())
    }
    
    fn authentication(&self, user_id: &UserId) -> Result<Option<AuthenticationInfo>, Self::Error> {
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
    
    fn update_authentication(&self, user_id: &UserId, auth_info: AuthenticationInfo) -> Result<Option<AuthenticationInfo>, Self::Error> {
        let mut table_locked = self.auth.lock()?;
        for record in table_locked.iter_mut() {
            if record.user_id == *user_id {
                let old_auth = record.phc_string.clone();
                record.phc_string = auth_info.phc_string;
                return Ok(Some(old_auth.into()))
            };
        };
        table_locked.push(AuthRecord{ user_id: user_id.clone(), phc_string: auth_info.phc_string });
        Ok(None)
    }
    
    fn create_user(&self, username: &str) -> Result<Option<UserId>, Self::Error> {
        let mut table_locked = self.users.lock()?;
        let mut last_user_id_locked = self.last_user_id.lock()?;

        if table_locked.iter().filter(|record| record.1 == username).next().is_some() {
            return Ok(None)
        };

        let user_id = format!("{last_user_id_locked}");


        table_locked.push((user_id.clone(), username.to_owned()));
        *last_user_id_locked += 1;
        Ok(Some(user_id))
    }
}
