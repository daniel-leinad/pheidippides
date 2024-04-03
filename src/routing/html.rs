use anyhow::Result;
use crate::serde_form_data;
use super::db::{self, UserId};
use crate::http::{Request, Response};
use serde::Deserialize;
use crate::utils::get_headers_hashmap;
use super::get_authorization;

struct HtmlString(String);

impl From<&db::ChatInfo> for HtmlString {
    fn from(value: &db::ChatInfo) -> Self {
        let id = &value.id;
        let username = &value.username;
        HtmlString(format!(
            "<div class=\"chat\" id=\"chat_{id}\" onclick=\"chatWith({id})\">{username}</div>"
        ))
    }
}

pub fn chats_html_response(request: &Request, db_access: impl db::DbAccess) -> Result<Response> {
    let headers = get_headers_hashmap(request);
    let authorization = get_authorization(headers)?;
    let response_string = match authorization {
        Some(user_id) => chats_html(&db_access, &user_id)?,
        None => String::from("Unauthorized"),
    };
    Ok(Response::Text(response_string))
}

pub fn chats_html(db_access: &impl db::DbAccess, user_id: &UserId) -> Result<String> {
    let res: String = db_access
        .chats(user_id)?
        .iter()
        .map(|chat_info| HtmlString::from(chat_info).0)
        .intersperse(String::from("\n"))
        .collect();
    Ok(res)
}

#[derive(Deserialize)]
struct ChatSearchParams {
    query: String,
}

pub fn chatsearch_html(db_access: impl db::DbAccess, params: &str) -> Result<Response> {

    let search_params: ChatSearchParams = match serde_form_data::from_str(params) {
        Ok(res) => res,
        Err(_) => return Ok(Response::Empty),
    };

    let chats_html: String = db_access
        .find_chats(&search_params.query)?
        .into_iter()
        .map(|chat_info| HtmlString::from(&chat_info).0)
        .intersperse("\n".to_owned())
        .collect();

    Ok(Response::Text(chats_html))

}