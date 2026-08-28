use std::error::Error;

use rust_reading_notes_api::{build_app, connect_database};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    let pool = connect_database("sqlite://reading-notes.db").await?;
    let app = build_app(pool);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    tracing::info!(address = %listener.local_addr()?, "server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
