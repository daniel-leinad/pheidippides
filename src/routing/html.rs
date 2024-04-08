use anyhow::{Context, Result};
use crate::{db::ChatInfo, serde_form_data};
use super::db::{self, UserId};
use crate::http::{Request, Response};
use serde::Deserialize;
use super::get_authorization;
use askama::Template;

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatPage<'a> {
    username: &'a str,
    chats: Vec<ChatInfo>,
}

#[derive(Template)]
#[template(path = "elements/chats.html")]
struct ChatHtmlElements {
    chats: Vec<ChatInfo>,
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {}

#[derive(Template)]
#[template(path = "signup.html")]
struct SignUpPage {}

#[derive(Template)]
#[template(path = "login_fail.html")]
struct LoginFailPage {}

pub async fn chat_page(db_access: &impl db::DbAccess, user_id: &UserId) -> Result<String> {
    let username = db_access
        .username(&user_id).await.with_context(|| format!("Couldn't fetch username of {user_id}"))?
        .context("Couldn't retrieve username from user_id stored SESSION_INFO")?;

    let chats = fetch_users_chats(db_access, user_id).await?;

    ChatPage{ username: &username, chats: chats }.render().context("Could not render chat.html")
}

pub fn login_page() -> Result<String> {
    LoginPage{}.render().context("Could not render login.html")
}

pub fn signup_page() -> Result<String> {
    SignUpPage{}.render().context("Could not render signup.html")
}

pub fn login_fail_page() -> Result<String> {
    LoginFailPage{}.render().context("Could not render login_fail.html")
}

pub async fn chats_html_response(request: &Request, db_access: impl db::DbAccess) -> Result<Response> {
    let headers = request.headers();
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&db_access, &user_id).await?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Html{content: response_string, headers: vec![]})
}

pub async fn chats_html(db_access: &impl db::DbAccess, user_id: &UserId) -> Result<String> {
    let chats = fetch_users_chats(db_access, user_id).await?;
    Ok(ChatHtmlElements{ chats }.render().context("Could not render elements/chats.html")?)
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

pub async fn chatsearch_html(db_access: impl db::DbAccess, params: &str) -> Result<Response> {

    let search_params: ChatSearchParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::Empty),
    };

    let chats = db_access
        .find_chats(&search_params.query).await
        .with_context(|| format!("Could't find chats with query {}", &search_params.query))?;

    let chats_html = ChatHtmlElements{chats}.render()?;

    Ok(Response::Html{content: chats_html, headers: vec![]})

}

async fn fetch_users_chats(db_access: &impl db::DbAccess, user_id: &UserId) -> Result<Vec<ChatInfo>> {
    let chats = db_access
            .chats(user_id).await
            .with_context(|| format!("Couldn't fetch chats for user {user_id}"))?;
    Ok(chats)
}