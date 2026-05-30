use axum::{
    extract::{Extension, Path, State},
    routing::{delete, get, patch},
    Json, Router,
};
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};

use crate::error::AppError;
use crate::models::{User, UserResponse};
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", get(get_user))
        .route("/api/users/{id}", patch(update_user))
        .route("/api/users/{id}", delete(delete_user))
}

/// Returns whether the given user id belongs to an administrator.
async fn is_admin(state: &AppState, user_id: &str) -> Result<bool, AppError> {
    let admin: Option<bool> = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(state.db.pool())
        .await?;
    Ok(admin.unwrap_or(false))
}

/// List all users (admin only)
async fn list_users(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    if !is_admin(&state, &user_id).await? {
        return Err(AppError::Unauthorized("Admin privileges required".into()));
    }

    let users: Vec<User> = sqlx::query_as(
        "SELECT * FROM users ORDER BY created_at DESC"
    )
    .fetch_all(state.db.pool())
    .await?;

    Ok(Json(users.into_iter().map(|u| u.into()).collect()))
}

/// Get user by ID (own profile or admin)
async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<UserResponse>, AppError> {
    // Requester must be viewing their own profile or be an admin
    if id != user_id && !is_admin(&state, &user_id).await? {
        return Err(AppError::Unauthorized("Can only view own profile".into()));
    }

    let user: User = sqlx::query_as(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(&id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    Ok(Json(user.into()))
}

/// Update user profile
#[derive(serde::Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub password: Option<String>,
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    // Users can update their own profile; admins can update anyone
    if id != user_id && !is_admin(&state, &user_id).await? {
        return Err(AppError::Unauthorized("Can only update own profile".into()));
    }

    let now = chrono::Utc::now();

    if let Some(password) = req.password {
        // Hash new password
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .to_string();

        sqlx::query("UPDATE users SET password_hash = $1, updated_at = $2 WHERE id = $3")
            .bind(&password_hash)
            .bind(now)
            .bind(&id)
            .execute(state.db.pool())
            .await?;
    }

    if let Some(email) = req.email {
        sqlx::query("UPDATE users SET email = $1, updated_at = $2 WHERE id = $3")
            .bind(&email)
            .bind(now)
            .bind(&id)
            .execute(state.db.pool())
            .await?;
    }

    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(&id)
        .fetch_one(state.db.pool())
        .await?;

    Ok(Json(user.into()))
}

/// Delete user (and all their files)
async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Users can delete their own account; admins can delete anyone
    if id != user_id && !is_admin(&state, &user_id).await? {
        return Err(AppError::Unauthorized("Can only delete own account".into()));
    }

    // Delete user's files (CASCADE handles this in DB)
    sqlx::query("DELETE FROM files WHERE user_id = $1")
        .bind(&id)
        .execute(state.db.pool())
        .await?;

    // Delete user's shares
    sqlx::query("DELETE FROM shares WHERE user_id = $1")
        .bind(&id)
        .execute(state.db.pool())
        .await?;

    // Delete user
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&id)
        .execute(state.db.pool())
        .await?;

    Ok(Json(serde_json::json!({"success": true})))
}