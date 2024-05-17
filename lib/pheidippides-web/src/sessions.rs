use std::sync::RwLock;
use std::collections::HashMap;

use anyhow::{bail, Result};
use once_cell::sync::Lazy;

use pheidippides_messenger::UserId;

pub type SessionId = String;

pub const SESSION_ID_COOKIE: &str = "_pheidippides_sid";
pub static SESSION_INFO: Lazy<RwLock<HashMap<SessionId, SessionInfo>>> = Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Clone)]
pub struct SessionInfo {
    pub user_id: UserId,
}

pub fn generate_session_id() -> SessionId {
    uuid::Uuid::new_v4().into()
}

pub fn update_session_info(session_id: SessionId, session_info: SessionInfo) -> Result<()> {
    match SESSION_INFO.write() {
        Ok(mut session_info_write_lock) => {
            session_info_write_lock.insert(session_id, session_info);
            Ok(())
        }
        Err(e) => {
            bail!("Could not lock SESSION_INFO global for write: {}", e)
        }
    }
}

pub fn get_session_info(session_id: &SessionId) -> Result<Option<SessionInfo>> {
    let res = match SESSION_INFO.read() {
        Ok(session_info_read_lock) => session_info_read_lock.get(session_id).cloned(),
        Err(e) => {
            bail!("Could not lock SESSION_INFO global for read: {}", e)
        }
    };
    Ok(res)
}

pub fn remove_session_info(session_id: &SessionId) -> Result<()> {
    match SESSION_INFO.write() {
        Ok(mut session_info_write_lock) => {
            session_info_write_lock.remove(session_id);
        },
        Err(e) => bail!("Could not lock SESSION_INFO global for write: {}", e),
    }
    Ok(())
}