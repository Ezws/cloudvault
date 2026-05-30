pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub db: db::Database,
}