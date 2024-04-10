use anyhow::{Context, Result};
use crate::{db::ChatInfo, serde_form_data};
use super::db::{self, UserId};
use crate::app::App;
use crate::http::{Request, Response};
use serde::Deserialize;
use super::get_authorization;
use askama::Template;

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatPage<'a> {
    username: &'a str,
    user_id: &'a UserId,
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

pub async fn chat_page(app: &App<impl db::DbAccess>, user_id: &UserId) -> Result<String> {
    let username = app
         .username(&user_id).await?
         .with_context(|| format!("Incorrect user id: {user_id}"))?;
    
    let users_chats = app.fetch_users_chats(user_id).await?;

    ChatPage{ username: &username, user_id, chats: users_chats }.render().context("Could not render chat.html")
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

pub async fn chats_html_response(request: &Request, app: App<impl db::DbAccess>) -> Result<Response> {
    let headers = request.headers();
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&app, &user_id).await?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Html{content: response_string, headers: vec![]})
}

pub async fn chats_html(app: &App<impl db::DbAccess>, user_id: &UserId) -> Result<String> {
    let chats = app.fetch_users_chats(user_id).await?;
    Ok(ChatHtmlElements{ chats }.render().context("Could not render elements/chats.html")?)
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

pub async fn chatsearch_html(app: App<impl db::DbAccess>, params: &str) -> Result<Response> {

    let search_params: ChatSearchParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::Empty),
    };

    let chats = app.find_chats(&search_params.query).await?;

    let chats_html = ChatHtmlElements{chats}.render()?;

    Ok(Response::Html{content: chats_html, headers: vec![]})

}