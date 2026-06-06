use axum::Router;

use super::AppState;

pub mod agent;
pub mod capability;
pub mod chat_flow;
pub mod eval;
pub mod foundation;
pub mod knowledge;
pub mod model;
pub mod template;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(agent::routes())
        .merge(capability::routes())
        .merge(eval::routes())
        .merge(foundation::routes())
        .merge(knowledge::routes())
        .merge(model::routes())
        .merge(template::routes())
}
