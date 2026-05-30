pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;

use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use axum::http::{HeaderValue, Method, header};

pub use crate::config::Config;
pub use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::{routing::get, Router};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cloudvault_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::load()?;
    tracing::info!("Configuration loaded");

    // Initialize database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database.url)
        .await?;
    
    let db = Database::new(pool);
    tracing::info!("Database connection established");

    // Create application state
    let state = AppState {
        config: config.clone(),
        db,
    };

    async fn health_check() -> &'static str {
        "OK"
    }

    // CORS layer - restrict to configured origins
    let allowed: Vec<HeaderValue> = config
        .cors
        .allowed_origins
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect();

    if allowed.is_empty() {
        tracing::warn!("No valid CORS origins configured; cross-origin browser requests will be blocked");
    } else {
        tracing::info!("CORS allowed origins: {}", config.cors.allowed_origins);
    }

    let cors = CorsLayer::new()
        .allow_origin(allowed)
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ]);

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .merge(routes::auth_routes())
        .merge(routes::files_routes())
        .merge(routes::users_routes())
        .merge(routes::shares_routes())
        .with_state(state.clone())
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(
            Arc::new(state),
            middleware::jwt_auth_layer,
        ));

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}