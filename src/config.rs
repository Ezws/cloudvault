use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub jwt: JwtConfig,
    pub storage: StorageConfig,
    pub cors: CorsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub local_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    /// Comma-separated list of allowed origins, e.g. "http://localhost:8081,http://127.0.0.1:8081"
    pub allowed_origins: String,
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        dotenvy::dotenv().ok();

        let mut settings = config::Config::builder();

        settings = settings
            .set_default("database.url", "postgres://cloudvault:password@localhost:5432/cloudvault")?
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", "8080")?
            .set_default("jwt.secret", "your-secret-key-change-in-production")?
            .set_default("jwt.expiration_hours", "24")?
            .set_default("storage.local_path", "./storage")?
            .set_default(
                "cors.allowed_origins",
                "http://localhost:8081,http://127.0.0.1:8081",
            )?;

        settings = settings
            .add_source(config::Environment::with_prefix("CV").separator("__"));

        settings.build()?.try_deserialize()
    }
}