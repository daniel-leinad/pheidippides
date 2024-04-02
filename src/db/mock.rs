use super::*;
use std::{collections::HashMap, sync::{Arc, Mutex, PoisonError}};

struct MessageRecord {
    from: UserId,
    to: UserId,
    message: String,
}

#[derive(Debug)]
pub enum Error {
    ThreadPoisonError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThreadPoisonError => write!(f, "Thread poisoning error")
        }
    }
}

impl std::error::Error for Error {}

impl<T> From<PoisonError<T>> for Error {
    fn from(_value: PoisonError<T>) -> Self {
        Self::ThreadPoisonError
    }
}

#[derive(Clone)]
pub struct Db {
    users: Arc<Vec<(UserId, String)>>,
    messages: Arc<Mutex<Vec<MessageRecord>>>,
}

impl Db {
    pub fn new() -> Self {
        let users = Arc::new(vec![
            ("1".into(), "Dan".into()),
            ("2".into(), "Man".into()),
            ("3".into(), "John".into()),
        ]);
        let messages = Arc::new(Mutex::new(vec![
            MessageRecord{ from: "1".into(), to: "1".into(), message: "Привет!".into() },
            MessageRecord{ from: "1".into(), to: "1".into(), message: "Ой, я написал самому себе...".into() },
            MessageRecord{ from: "2".into(), to: "1".into(), message: "Hey".into() },
            MessageRecord{ from: "1".into(), to: "2".into(), message: "Hey, man..".into() },
            MessageRecord{ from: "2".into(), to: "1".into(), message: "Actually, I AM Man...".into() },
            MessageRecord{ from: "1".into(), to: "2".into(), message: "Right... 😂😂😂".into() },
            MessageRecord{ from: "1".into(), to: "3".into(), message: "Hey, John, like your new song!".into() },
            MessageRecord{ from: "3".into(), to: "1".into(), message: "Thanks, it's very popular, can you imagine that?".into() },
        ]));
        Db { users, messages }
    }
}

impl DbAccess for Db {
    type Error = Error;
    fn users(&self) -> Result<Vec<(UserId, String)>, Error> {
        Ok(self.users.iter().map(|value| value.clone()).collect())
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
                Some(ChatInfo::new(users.get(&msg_record.to).unwrap_or(&"<unknown user id>".to_owned()).clone(), msg_record.to.clone()))
            } else if &msg_record.to == user_id {
                Some(ChatInfo::new(users.get(&msg_record.from).unwrap_or(&"<unknown user id>".to_owned()).clone(), msg_record.from.clone()))
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

    fn messages(&self, this: &UserId, other: &UserId) -> Result<Vec<Message>, Error> {
        let res = self.messages.lock()?.iter().filter_map(|msg_record| {
            if &msg_record.from == this && &msg_record.to == other {
                Some(Message::Out(msg_record.message.clone()))
            } else if &msg_record.from == other && &msg_record.to == this {
                Some(Message::In(msg_record.message.clone()))
            } else {
                None
            }
        }).collect();
        Ok(res)
    }
    
    fn create_message(&self, message: String, from: &UserId, to: &UserId) -> Result<(), Self::Error> {
        let mut messages_lock = self.messages.lock()?;
        messages_lock.push(MessageRecord { from: from.into() , to: to.into() , message });
        Ok(())
    }
}
