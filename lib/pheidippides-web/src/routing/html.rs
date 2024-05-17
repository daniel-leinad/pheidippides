use anyhow::{Context, Result};
use serde::Deserialize;
use askama::Template;
use tokio::io::AsyncRead;

use http_server::response::Response;
use http_server::request::Request;

use pheidippides_utils::serde::form_data as serde_form_data;

use pheidippides_messenger::{User, UserId};
use pheidippides_messenger::data_access::DataAccess;
use pheidippides_messenger::messenger::Messenger;

use crate::routing::get_authorization;

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatPage<'a> {
    username: &'a str,
    user_id: &'a UserId,
    chats: Vec<User>,
}

#[derive(Template)]
#[template(path = "elements/chats.html")]
struct ChatHtmlElements {
    chats: Vec<User>,
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

pub async fn chat_page<A>(app: &Messenger<impl DataAccess, A>, user_id: &UserId) -> Result<String> {
    let username= app
        .fetch_user(&user_id).await?
        .with_context(|| format!("Incorrect user id: {user_id}"))?
        .username;
    
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

pub async fn chats_html_response<A, T: AsyncRead + Unpin>(request: &Request<T>, app: Messenger<impl DataAccess, A>) -> Result<Response> {
    let headers = request.headers();
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&app, &user_id).await?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Html{content: response_string, headers: vec![]})
}

pub async fn chats_html<A>(app: &Messenger<impl DataAccess, A>, user_id: &UserId) -> Result<String> {
    let chats = app.fetch_users_chats(user_id).await?;
    Ok(ChatHtmlElements{ chats }.render().context("Could not render elements/chats.html")?)
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

pub async fn chatsearch_html<A>(app: Messenger<impl DataAccess, A>, params: &str) -> Result<Response> {

    let search_params: ChatSearchParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::Empty),
    };

    let chats = app.find_users_by_substring(&search_params.query).await?;

    let chats_html = ChatHtmlElements{chats}.render()?;

    Ok(Response::Html{content: chats_html, headers: vec![]})

}

pub async fn chat_html_response<A>(app: Messenger<impl DataAccess, A>, chat_id: &str) -> Result<Response> {
    // TODO authorization first??
    
    let chat_id: UserId = match chat_id.parse() {
        Ok(res) => res,
        Err(_) => return Ok(Response::BadRequest),
    };

    let chat_info = app.fetch_user(&chat_id).await?;

    let res = ChatHtmlElements{chats: chat_info.into_iter().collect()}.render()?;

    Ok(Response::Html { content: res, headers: vec![] })
}