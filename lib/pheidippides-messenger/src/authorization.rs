use std::future::Future;

use crate::UserId;

//TODO code duplication
macro_rules! async_result {
    ($t:ty) => {
        impl Future<Output = Result<$t, Self::Error>> + Send
    };
}

pub trait AuthService: 'static + Send + Sync + Clone {
    type Error: 'static + std::error::Error + Send + Sync;

    fn verify_user(&self, user_id: &UserId, password: String) -> async_result!(bool);
    fn create_user(&self, user_id: &UserId, password: String) -> async_result!(());
}