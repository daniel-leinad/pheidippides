use sqlx::postgres::PgConnectOptions;
use sqlx::{Executor, Row, query};
use uuid::Uuid;
use chrono::{DateTime, Local};
use anyhow::{Context, Result, bail};
use thiserror::Error;

use super::{AuthenticationInfo, ChatInfo, DbAccess, Message, MessageType, UserId, MessageId};

const MESSAGE_LOAD_BUF_SIZE: i32 = 50;
const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
const DB_VERSION: i64 = 2;

#[derive(Clone)]
pub struct Db {
    pool: sqlx::PgPool,
}

impl Db {
    pub async fn new(connection_string: &str) -> Result<Self> {
        let options: PgConnectOptions = connection_string.parse()?;
        let pool = sqlx::PgPool::connect_with(options).await?;

        Ok(Db { pool })
    }

    pub async fn check_migrations(&self) -> Result<()> {
        let migrations_table_exists: bool = self.pool
            .acquire().await?
            .fetch_one(query("select exists (select from pg_tables where schemaname = 'public' and tablename = '_sqlx_migrations')"))
            .await?
            .get(0);

        if !migrations_table_exists {
            bail!("Database uninitialized. Please migrate database using the 'migrate' tool");
        }
        
        let latest_version: i64 = self.pool
            .acquire().await?
            .fetch_optional(query("select version from _sqlx_migrations order by version desc limit 1"))
            .await?
            .map(|row| row.get(0))
            .unwrap_or(-1);

        if latest_version < DB_VERSION {
            bail!("Database schema not up to date. Please migrate database using the 'migrate' tool")
        } else if latest_version > DB_VERSION {
            bail!("Application not up to date with the database. Please use a newer version of the app or undo database migrations until version {}", DB_VERSION)
        };

        Ok(())
    }

    pub async fn migrate(&self) -> Result<()> {
        MIGRATOR.run(&self.pool).await.context("Couldn't migrate")
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Postgres error: {0}")]
    PgError(#[from] sqlx::Error),
    #[error("Auth info parsing error: {0}")]
    AuthInfoParsingError(#[from] super::AuthenticationInfoParsingError),
}

impl DbAccess for Db {
    type Error = Error;

    async fn users(&self) -> Result<Vec<(UserId, String)>, Self::Error> {

        let res = self.pool.acquire().await?
        .fetch_all("select user_id, username from users").await?
            .iter()
            .map(|row| {
                (row.get(0), row.get(1))
            })
            .collect();

        Ok(res)
    }
    
    async fn chats(&self, user_id: &UserId) -> Result<Vec<ChatInfo>, Self::Error> {
        let mut conn = self.pool.acquire().await?;

        let temp_table_name = pg_id(&format!("temp_chat_ids_{}", Uuid::new_v4()));
        conn.execute(query(&format!(r#"
            create temp table {temp_table_name} as 
            select distinct 
                sender as user_id
            from messages 
            where receiver = $1 
            
            union 
            
            select 
                receiver as user_id
            from messages
            where sender = $1
            "#)).bind(user_id)).await?;

        let res = conn.fetch_all(query(&format!(r#"
            select
                user_id,
                username
            from users
            where user_id in (select user_id from {temp_table_name})
            "#))).await?
            .iter()
            .map(|row| {ChatInfo{id: row.get(0), username: row.get(1)}})
            .collect();

        conn.execute(query(&format!("drop table {temp_table_name};"))).await?;

        Ok(res)
    }
    
    async fn last_messages(&self, this: &UserId, other: &UserId, starting_point: Option<MessageId>)-> Result<Vec<Message>, Self::Error> {
        let mut conn = self.pool.acquire().await?;
        let mut query_builder = sqlx::QueryBuilder::new("");
        query_builder.push(r#"
            select id, receiver, message
            from messages
            where (receiver = "#).push_bind(this)
        .push(" and sender = ").push_bind(other)
        .push(") or (receiver = ").push_bind(other)
        .push(" and sender = ").push_bind(this)
        .push(")");
        
        if let Some(starting_point) = starting_point {
            let timestamp = conn
                .fetch_optional(query("select timestamp from messages where id = $1").bind(starting_point)).await?;
            //TODO possibly handle case when timestamp is none
            if let Some(pg_row) = timestamp {
                let timestamp: DateTime<Local> = pg_row.get(0);
                query_builder
                    .push(" and timestamp <= ").push_bind(timestamp)
                    .push(" and id < ").push_bind(starting_point);
            }
        }

        query_builder.push(" order by timestamp desc");
        query_builder.push(" limit ").push_bind(MESSAGE_LOAD_BUF_SIZE);

        let query = query_builder.build();
        let res = conn.fetch_all(query).await?
            .iter()
            .map(|row| {
                let id = row.get(0);
                let to: UserId = row.get(1);
                let message = row.get(2);
                let message_type = if to == *this {MessageType::In} else {MessageType::Out};
                Message{id, message_type, message}
            })
            .collect();
        Ok(res)
    }
    
    async fn create_message(&self, msg: String, from: &UserId, to: &UserId) -> Result<MessageId, Self::Error> {
        let mut conn = self.pool.acquire().await?;
        let message_id = Uuid::new_v4();
        let timestamp = Local::now();
        conn.execute(query(r#"
                insert into messages(id, sender, receiver, message, timestamp)
                values ($1, $2, $3, $4, $5)
            "#)
            .bind(message_id)
            .bind(from)
            .bind(to)
            .bind(msg)
            .bind(timestamp))
            .await?;
        Ok(message_id)
    }
    
    async fn authentication(&self, user_id: &UserId) -> Result<Option<AuthenticationInfo>, Self::Error> {
        let res = self.pool.acquire().await?
            .fetch_optional(query(r#"
            select phc_string from auth where user_id = $1
            "#).bind(user_id)).await?;
        
        match res {
            Some(row) => {
                let phc_string: &str = row.get(0);
                let auth_info = phc_string.parse()?;
                Ok(Some(auth_info))
            },
            None => Ok(None),
        }
    }
    
    async fn update_authentication(&self, user_id: &UserId, auth_info: super::AuthenticationInfo) -> Result<Option<AuthenticationInfo>, Self::Error> {
        let mut transaction = self.pool.begin().await?;
        transaction.execute(query("lock table auth in exclusive mode")).await?;
        let old_auth = transaction.fetch_optional(query(
                "select phc_string from auth where user_id = $1"
            ).bind(user_id)).await?;

        match old_auth {
            Some(row) => {
                let old_phc_string: &str = row.get(0);
                let old_auth: AuthenticationInfo = old_phc_string.parse()?;
                transaction.execute(query(
                    "update auth set phc_string = $1 where user_id = $2"
                ).bind(auth_info.phc_string().to_string()).bind(user_id)).await?;
                transaction.commit().await?;
                Ok(Some(old_auth))
            },
            None => {
                transaction.execute(query(r#"
                    insert into auth (user_id, phc_string) values ($1, $2)
                    "#).bind(user_id).bind(auth_info.phc_string.to_string())).await?;
                transaction.commit().await?;
                Ok(None)
            },
        }
    }
    
    async fn create_user(&self, username: &str) -> Result<Option<UserId>, Self::Error> {
        let user_id = Uuid::new_v4();
        let mut transaction = self.pool.begin().await?;

        transaction.execute("lock table users in exclusive mode;").await?;

        let username_exists: bool = transaction
            .fetch_one(query(r#"
                select exists(select 1 from users where lower(username) = $1)
            "#).bind(username.to_lowercase())).await?.get(0);
        
        if username_exists {
            return Ok(None);
        };
        
        transaction.execute(query(r#"
                insert into users(user_id, username) values ($1, $2);
            "#).bind(user_id).bind(username)).await?;
        
        transaction.commit().await?;

        Ok(Some(user_id))
    }

    async fn username(&self, user_id: &UserId) -> Result<Option<String>, Error> {
        let mut conn = self.pool.acquire().await?;
        let res = conn.fetch_optional(query(r#"
            select username from users where user_id = $1
        "#).bind(user_id)).await?;
        Ok(res.map(|row| row.get(0)))
    }

    async fn user_id(&self, requested_username: &String) -> Result<Option<UserId>, Error> {
        let mut conn = self.pool.acquire().await?;
        let res = conn.fetch_optional(query(r#"
            select user_id from users where lower(username) = $1
        "#).bind(requested_username.to_lowercase())).await?;
        Ok(res.map(|row| row.get(0))) 
    }
    
    async fn find_chats(&self, search_query: &str) -> Result<Vec<ChatInfo>, Error> {
        let mut conn = self.pool.acquire().await?;
        let res = conn.fetch_all(query(r#"
                select user_id, username from users where lower(username) like $1
            "#).bind(format!("%{}%", search_query.to_lowercase()))).await?
            .into_iter()
            .map(|row| {
                let id = row.get(0);
                let username = row.get(1);
                ChatInfo{ username, id }
            })
            .collect();
        Ok(res)
    }
}

//TODO possibly use Cow for optimization
fn pg_id(input: &str) -> String {
    format!("\"{}\"", input.replace("\"", "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_id_sanitizes_input() {
        let input = "table";
        assert_eq!(pg_id(input), r#""table""#);

        let input = "моя таблица";
        assert_eq!(pg_id(input), r#""моя таблица""#);

        let input = r#"моя "таблица""#;
        assert_eq!(pg_id(input), r#""моя ""таблица""""#);

        let input = r#"weird table , - ; " : "#;
        assert_eq!(pg_id(input), r#""weird table , - ; "" : ""#);
    }
}