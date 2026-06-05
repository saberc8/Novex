use axum::Router;

use super::AppState;

pub mod foundation;
pub mod knowledge;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(foundation::routes())
        .merge(knowledge::routes())
}
