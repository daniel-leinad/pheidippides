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
use crate::flow_controller::HttpResponseContextExtension;

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

pub async fn chat_page<A>(app: &Messenger<impl DataAccess, A>, user_id: &UserId) -> Result<Option<String>> {
    let user= app.fetch_user(user_id).await?;

    let username = match user {
        Some(user) => user.username,
        None => return Ok(None),
    };

    let users_chats = app.fetch_users_chats(user_id).await?;

    Ok(Some(
        ChatPage{
            user_id,
            username: &username,
            chats: users_chats
        }.render().context("Could not render chat.html")?
    ))
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

pub async fn chats_html_response<A, T: AsyncRead + Unpin>(request: &Request<T>, app: Messenger<impl DataAccess, A>) -> Response {
    let headers = request.headers();
    let authorization = get_authorization(headers).or_bad_request()?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&app, &user_id).await.or_server_error()?,
        None => String::from("Unauthorized"),
    };

    Response::Html{content: response_string, headers: vec![]}
}

pub async fn chats_html<A>(app: &Messenger<impl DataAccess, A>, user_id: &UserId) -> Result<String> {
    let chats = app.fetch_users_chats(user_id).await?;
    ChatHtmlElements{ chats }.render().context("Could not render elements/chats.html")
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

pub async fn chatsearch_html<A>(app: Messenger<impl DataAccess, A>, params: &str) -> Response {

    // TODO check authorization

    let search_params: ChatSearchParams = serde_form_data::from_str(params).or_bad_request()?;

    let chats = app.find_users_by_substring(&search_params.query).await.or_server_error()?;

    let chats_html = ChatHtmlElements{chats}.render().or_server_error()?;

    Response::Html{content: chats_html, headers: vec![]}

}

pub async fn chat_html_response<A>(app: Messenger<impl DataAccess, A>, chat_id: &str) -> Response {
    // TODO authorization first??
    
    let chat_id: UserId = chat_id.parse().or_bad_request()?;
    let chat_info = app.fetch_user(&chat_id).await.or_server_error()?;
    let res = ChatHtmlElements{chats: chat_info.into_iter().collect()}.render().or_server_error()?;

    Response::Html { content: res, headers: vec![] }
}