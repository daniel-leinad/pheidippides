use std::future::Future;
use anyhow::{bail, Context, Result};

use argon2::{
    Argon2,
    password_hash::{
        PasswordHasher,
        PasswordVerifier, rand_core::OsRng, SaltString
    }
};
use std::str::FromStr;
use std::result;
use thiserror::Error;

use crate::UserId;

macro_rules! async_result {
    ($t:ty) => {
        impl Future<Output = Result<$t, Self::Error>> + Send
    };
}

pub trait AuthStorage: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;
    fn fetch_authentication(&self, user_id: &UserId) -> async_result!(Option<AuthenticationInfo>);
    fn update_authentication(&self, user_id: &UserId, auth_info: AuthenticationInfo) -> async_result!(Option<AuthenticationInfo>);
}

#[derive(Clone)]
pub struct AuthService<A> {
    storage: A
}

impl<A> AuthService<A> {
    pub fn new(storage: A) -> Self {
        Self { storage }
    }
}

impl<A: AuthStorage> AuthService<A> {
    pub async fn verify_user(&self, user_id: &UserId, password: String) -> Result<bool> {
        let auth_info = match self.storage
            .fetch_authentication(user_id).await
            .with_context(|| format!("Couldn't fetch authentification for {user_id}"))? {
            Some(auth_info) => auth_info,
            None => return Ok(false),
        };

        let handle = tokio::task::spawn_blocking(move || {
            let password_hash = auth_info.phc_string().password_hash();
            Argon2::default().verify_password(password.as_bytes(), &password_hash).is_ok()
        });

        let res = handle.await.context("Password verification thread failed")?;
        Ok(res)
    }

    pub async fn create_user(&self, user_id: &UserId, password: String) -> Result<()> {
        let handle = tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(OsRng);
            let password_hash = match Argon2::default().hash_password(password.as_bytes(), &salt) {
                Ok(hash) => hash,
                Err(e) => bail!("Couldn't generate hash from {password}: {e}"),
            };
            Ok(AuthenticationInfo::from(password_hash))
        });

        let auth_info = handle.await.context("Password hash generation thread failed")??;
        self.storage.update_authentication(user_id, auth_info).await.with_context(|| format!("Couldn't update authentification for {user_id}"))?; //TODO check if user already existed?
        Ok(())
    }

}


// TODO auth logic does not belong here
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

impl FromStr for AuthenticationInfo {
    type Err = AuthenticationInfoParsingError;

    fn from_str(s: &str) -> result::Result<Self, Self::Err> {
        match s.parse() {
            Ok(phc_string) => Ok(AuthenticationInfo{phc_string}),
            Err(_) => Err(AuthenticationInfoParsingError::IncorrectPHCString(s.to_owned()))
        }
    }
}

#[derive(Error, Debug)]
pub enum AuthenticationInfoParsingError {
    #[error("Incorrect phc string: {0}")]
    IncorrectPHCString(String),
}
