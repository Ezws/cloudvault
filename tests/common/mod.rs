use cloudvault_server::{config::Config, db::Database, AppState};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub struct TestSetup {
    pub state: Arc<AppState>,
    pub config: Config,
    pub user_id: String,
    pub token: String,
}

impl TestSetup {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::load()?;
        
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database.url)
            .await?;
        
        let db = Database::new(pool);
        let state = Arc::new(AppState { config: config.clone(), db });
        
        Ok(Self {
            state,
            config,
            user_id: String::new(),
            token: String::new(),
        })
    }
    
    pub async fn create_user(&mut self, username: &str, password: &str) -> Result<(), Box<dyn std::error::Error>> {
        use argon2::{
            password_hash::{rand_core::OsRng, SaltString},
            Argon2, PasswordHasher,
        };
        use chrono::Utc;
        use uuid::Uuid;
        
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("hash error: {}", e))?
            .to_string();
        
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        sqlx::query(
            r#"INSERT INTO users (id, username, password_hash, storage_quota, storage_used, created_at, updated_at) 
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#
        )
        .bind(&user_id)
        .bind(username)
        .bind(&password_hash)
        .bind(10 * 1024 * 1024 * 1024i64)
        .bind(0i64)
        .bind(now)
        .bind(now)
        .execute(self.state.db.pool())
        .await?;
        
        self.user_id = user_id;
        
        // Generate token
        let expiration = Utc::now()
            .checked_add_signed(chrono::Duration::hours(24))
            .unwrap()
            .timestamp() as usize;
        
        let claims = Claims {
            sub: self.user_id.clone(),
            exp: expiration,
        };
        
        self.token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt.secret.as_bytes()),
        )?;
        
        Ok(())
    }
    
    pub async fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.user_id.is_empty() {
            sqlx::query("DELETE FROM files WHERE user_id = $1")
                .bind(&self.user_id)
                .execute(self.state.db.pool())
                .await?;
            sqlx::query("DELETE FROM shares WHERE user_id = $1")
                .bind(&self.user_id)
                .execute(self.state.db.pool())
                .await?;
            sqlx::query("DELETE FROM users WHERE id = $1")
                .bind(&self.user_id)
                .execute(self.state.db.pool())
                .await?;
        }
        Ok(())
    }
}

impl Drop for TestSetup {
    fn drop(&mut self) {
        // Note: async cleanup in Drop is tricky, we handle it manually
    }
}