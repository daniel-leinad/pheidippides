use crate::UserId;

use pheidippides_utils::async_result;

pub trait AuthService: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    fn verify_user(&self, user_id: &UserId, password: String) -> async_result!(bool);
    fn create_user(&self, user_id: &UserId, password: String) -> async_result!(());
}
