use crate::db::{AuthenticationInfo, DbAccess, UserId};
use anyhow::{bail, Context, Result};

use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

pub async fn verify_user<D: DbAccess>(user_id: &UserId, password: String, db_access: &D) -> Result<bool> {
    let auth_info = match db_access
        .authentication(user_id).await
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

pub async fn create_user<D: DbAccess>(user_id: &UserId, password: String, db_access: &D) -> Result<()> {
    let handle = tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(OsRng);
        let password_hash = match Argon2::default().hash_password(password.as_bytes(), &salt) {
            Ok(hash) => hash,
            Err(e) => bail!("Couldn't generate hash from {password}: {e}"),
        };
        Ok(AuthenticationInfo::from(password_hash))
    });

    let auth_info = handle.await.context("Password hash generation thread failed")??;
    db_access.update_authentication(user_id, auth_info).await.with_context(|| format!("Couldn't update authentification for {user_id}"))?; //TODO check if user already existed?
    Ok(())
}