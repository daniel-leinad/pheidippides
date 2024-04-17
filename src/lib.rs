#![feature(slice_split_once)]
#![feature(iter_intersperse)]


pub mod http;
pub mod routing;
pub mod app;
pub mod db;

mod authorization;
mod serde_form_data;
mod utils;
mod async_utils;
mod sessions;