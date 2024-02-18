use axum::{routing::post, Router};
use bitespeed_identity_reconciliation::handler_identify;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() {
    // connect to the database.
    let database_url = "sqlite://bitespeed.sqlite3";
    let pool = SqlitePoolOptions::new()
        .connect(&database_url)
        .await
        .unwrap_or_else(|_| panic!("connect to sqlite db: {}", database_url));

    // bind to port and serve the app.
    let listener = tokio::net::TcpListener::bind(&format!("{}:{}", "127.0.0.1", 38001))
        .await
        .unwrap();

    let app = Router::new()
        .route("/identify", post(handler_identify))
        .with_state(pool);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
