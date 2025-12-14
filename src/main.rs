mod error;
mod types;
mod engine;
mod interp;
mod ledger;
mod trust_barrier;
mod api;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use crate::ledger::Ledger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🚀 UBL Kernel 2.1.0 Starting...");
    let ledger = Arc::new(Ledger::new());

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/register", post(api::register))
        .route("/execute", post(api::execute))
        .route("/verify", post(api::verify))
        .route("/registry/chips", get(api::list_chips))
        .route("/registry/programs", get(api::list_programs))
        .route("/barrier/process", post(api::barrier_process))
        .layer(CorsLayer::permissive())
        .with_state(ledger);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    info!("✅ Listening on 0.0.0.0:8000");
    axum::serve(listener, app).await?;

    Ok(())
}
