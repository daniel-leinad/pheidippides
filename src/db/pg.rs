use std::future::Future;

use sqlx::postgres::PgConnectOptions;
use sqlx::{database, query, Executor, Row};
use tokio::task::JoinError;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use chrono::{DateTime, Local};
use anyhow::{Context, Result, bail};
use thiserror::Error;

use super::{AuthenticationInfo, ChatInfo, DbAccess, Message, UserId, MessageId};

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

    pub fn graceful_shutdown(&self, cancellation_token: CancellationToken) -> impl Future<Output = Result<(), JoinError>> {
        let pool_cloned = self.pool.clone();
        let res = tokio::spawn(async move {
            cancellation_token.cancelled().await;
            eprintln!("Shutting down database connection...");
            pool_cloned.close().await;
            eprintln!("Shutting down database connection...Success");
        });
        res
    }

    pub async fn check_migrations(&self) -> Result<()> {
        let migrations_table_exists: bool = self.pool
            .acquire().await?
            .fetch_one(query("select exists (select from pg_tables where (schemaname = 'public') and (tablename = '_sqlx_migrations'))"))
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

        let temp_table_chat_ids = temp_table_name("chat_ids");
        conn.execute(query(&format!(r#"
            create temp table {temp_table_chat_ids} as 
            select
                sender as user_id,
                MAX(timestamp) as timestamp
            from messages 
            where receiver = $1
            group by user_id 
            
            union 
            
            select 
                receiver as user_id,
                MAX(timestamp) as timestamp
            from messages
            where sender = $1
            group by user_id
            "#)).bind(user_id)).await?;
        
        let temp_table_chat_ids_grouped = temp_table_name("chat_ids_grouped");
        conn.execute(query(&format!(r#"
            create temp table {temp_table_chat_ids_grouped} as
            select
                user_id as user_id, 
                MAX(timestamp) as timestamp
            from {temp_table_chat_ids}
            group by user_id
            "#))).await?;

        conn.execute(query(&format!("drop table {temp_table_chat_ids};"))).await?;

        let res = conn.fetch_all(query(&format!(r#"
            select
                last_messages.user_id,
                users.username
            from
            {temp_table_chat_ids_grouped} as last_messages
                left join users as users on last_messages.user_id = users.user_id
            order by last_messages.timestamp desc
            "#))).await?
            .iter()
            .map(|row| {ChatInfo{id: row.get(0), username: row.get(1)}})
            .collect();

        conn.execute(query(&format!("drop table {temp_table_chat_ids_grouped};"))).await?;

        Ok(res)
    }
    
    async fn last_messages(&self, user_id_1: &UserId, user_id_2: &UserId, starting_point: Option<MessageId>)-> Result<Vec<Message>, Self::Error> {
        let mut conn = self.pool.acquire().await?;
        let mut query_builder = sqlx::QueryBuilder::new("");
        query_builder.push(r#"
            select id, sender, receiver, message, timestamp
            from messages
            where ((receiver = "#).push_bind(user_id_1)
        .push(" and sender = ").push_bind(user_id_2)
        .push(") or (receiver = ").push_bind(user_id_2)
        .push(" and sender = ").push_bind(user_id_1)
        .push("))");
        
        if let Some(starting_point) = starting_point {
            let msg_timestamp = conn
                .fetch_optional(query("select timestamp from messages where id = $1").bind(starting_point)).await?;
            //TODO possibly handle case when timestamp is none
            if let Some(pg_row) = msg_timestamp {
                let msg_timestamp: DateTime<Local> = pg_row.get(0);
                query_builder
                    .push(" and (timestamp <= ").push_bind(msg_timestamp)
                    .push(") and (id < ").push_bind(starting_point)
                    .push(")");
            }
        }

        query_builder.push(" order by timestamp desc");
        query_builder.push(" limit ").push_bind(MESSAGE_LOAD_BUF_SIZE);

        let query = query_builder.build();
        let res = conn.fetch_all(query).await?
            .iter()
            .map(|row| {
                let id: MessageId = row.get(0);
                let from: UserId = row.get(1);
                let to: UserId = row.get(2);
                let message: String = row.get(3);
                let timestamp: DateTime<chrono::Utc> = row.get(4);
                Message{ id, from, to, message, timestamp }
            })
            .collect();
        Ok(res)
    }
    
    async fn create_message(&self, message: &Message) -> Result<(), Self::Error> {
        let mut conn = self.pool.acquire().await?;
        conn.execute(query(r#"
                insert into messages(id, sender, receiver, message, timestamp)
                values ($1, $2, $3, $4, $5)
            "#)
            .bind(message.id)
            .bind(message.from)
            .bind(message.to)
            .bind(&message.message)
            .bind(message.timestamp))
            .await?;
        Ok(())
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

    async fn user_id(&self, requested_username: &str) -> Result<Option<UserId>, Error> {
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

fn temp_table_name(name: &str) -> String {
    pg_id(&format!("temp_{name}_{}", Uuid::new_v4()))
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