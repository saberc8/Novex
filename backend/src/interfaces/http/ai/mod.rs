use axum::Router;

use super::AppState;

pub mod agent;
pub mod capability;
pub mod eval;
pub mod foundation;
pub mod knowledge;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(agent::routes())
        .merge(capability::routes())
        .merge(eval::routes())
        .merge(foundation::routes())
        .merge(knowledge::routes())
}
