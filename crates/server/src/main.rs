//! Authoritative game server. Games live in memory and are anonymous (joined by link);
//! the WebSocket game protocol comes with the first online-play story.

use axum::{Router, routing::get};

pub fn app() -> Router {
    Router::new().route("/health", get(|| async { "ok" }))
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("bind");
    axum::serve(listener, app()).await.expect("serve");
}
