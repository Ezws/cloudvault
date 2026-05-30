use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

use crate::error::AppError;
use crate::models::{CreateUserRequest, User, UserResponse};
use crate::AppState;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
}

#[derive(Debug, serde::Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    token: String,
    user: UserResponse,
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let password_hash = hash_password(&req.password)?;

    let mut user = User::new(req.username, password_hash);
    user.email = req.email.clone();

    // The first registered user becomes the administrator.
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(state.db.pool())
        .await?;
    user.is_admin = user_count == 0;

    sqlx::query(
        r#"INSERT INTO users (id, username, password_hash, email, storage_quota, storage_used, is_admin, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(&user.id)
    .bind(&user.username)
    .bind(&user.password_hash)
    .bind(&user.email)
    .bind(user.storage_quota)
    .bind(user.storage_used)
    .bind(user.is_admin)
    .bind(user.created_at)
    .bind(user.updated_at)
    .execute(state.db.pool())
    .await?;

    Ok(Json(user.into()))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let user: User = sqlx::query_as(
        "SELECT * FROM users WHERE username = $1"
    )
    .bind(&req.username)
    .fetch_one(state.db.pool())
    .await?;

    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(state.config.jwt.expiration_hours as i64))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.id.clone(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt.secret.as_bytes()),
    ).map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(LoginResponse {
        token,
        user: user.into(),
    }))
}

async fn me(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<String>,
) -> Result<Json<UserResponse>, AppError> {
    let user: User = sqlx::query_as(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(&user_id)
    .fetch_one(state.db.pool())
    .await?;

    Ok(Json(user.into()))
}

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .to_string();
    Ok(password_hash)
}

fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|_| AppError::Internal("Invalid password hash".into()))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

use jsonwebtoken::{encode, EncodingKey, Header};