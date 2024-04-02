use super::db::UserId;

const USERS: [&str; 4] = ["1", "2", "3", "4"];

pub fn validate_user_info(user_id: &UserId, _password: &str) -> bool {
    USERS.contains(&user_id.as_str())
}