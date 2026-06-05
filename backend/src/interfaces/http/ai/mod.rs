use axum::Router;

use super::AppState;

pub mod foundation;

pub fn routes() -> Router<AppState> {
    Router::new().merge(foundation::routes())
}
