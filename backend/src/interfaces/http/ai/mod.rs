use axum::Router;

use super::AppState;

pub mod capability;
pub mod foundation;
pub mod knowledge;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(capability::routes())
        .merge(foundation::routes())
        .merge(knowledge::routes())
}
