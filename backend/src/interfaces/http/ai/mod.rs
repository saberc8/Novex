use axum::Router;

use super::AppState;

pub mod agent;
pub mod capability;
pub mod chat_flow;
pub mod eval;
pub mod foundation;
pub mod integration;
pub mod knowledge;
pub mod memory;
pub mod model;
pub mod notebook;
pub mod studio;
pub mod template;
pub mod training;
pub mod trigger;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(agent::routes())
        .merge(capability::routes())
        .merge(chat_flow::routes())
        .merge(eval::routes())
        .merge(foundation::routes())
        .merge(integration::routes())
        .merge(knowledge::routes())
        .merge(memory::routes())
        .merge(model::routes())
        .merge(notebook::routes())
        .merge(studio::routes())
        .merge(template::routes())
        .merge(trigger::routes())
        .merge(training::routes())
}
