pub(crate) mod catalog;
mod ui;
pub mod api;

pub use api::{AppState, AdminKey, ServerCommonCors, create_router};
