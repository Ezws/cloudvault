use axum::{
    extract::{Extension, Path, State},
    routing::{delete, get, post},
    Json, Router,
};

use crate::error::AppError;
use crate::models::{CreateShareRequest, Share, ShareResponse};
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/shares", post(create_share))
        .route("/api/shares", get(list_shares))
        .route("/api/shares/{id}", delete(delete_share))
        .route("/api/shares/public/{token}", get(get_share_by_token))
}

/// Create a new share link
async fn create_share(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
    Json(req): Json<CreateShareRequest>,
) -> Result<Json<ShareResponse>, AppError> {
    // Verify file exists and belongs to user
    let _file: crate::models::File = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2"
    )
    .bind(&req.file_id)
    .bind(&user_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    let share = Share::new(req.file_id, user_id, &req.permissions);

    sqlx::query(
        r#"INSERT INTO shares (id, file_id, user_id, token, password, expires_at, permissions, created_at) 
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(&share.id)
    .bind(&share.file_id)
    .bind(&share.user_id)
    .bind(&share.token)
    .bind(&share.password)
    .bind(&share.expires_at)
    .bind(&share.permissions)
    .bind(&share.created_at)
    .execute(state.db.pool())
    .await?;

    Ok(Json(share.into()))
}

/// List all shares for current user
async fn list_shares(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
) -> Result<Json<Vec<ShareResponse>>, AppError> {
    let shares: Vec<Share> = sqlx::query_as(
        "SELECT * FROM shares WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(&user_id)
    .fetch_all(state.db.pool())
    .await?;

    Ok(Json(shares.into_iter().map(|s| s.into()).collect()))
}

/// Delete a share
async fn delete_share(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query("DELETE FROM shares WHERE id = $1 AND user_id = $2")
        .bind(&id)
        .bind(&user_id)
        .execute(state.db.pool())
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Share not found".into()));
    }

    Ok(Json(serde_json::json!({"success": true})))
}

/// Get share by public token (no auth required)
async fn get_share_by_token(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let share: Share = sqlx::query_as(
        "SELECT * FROM shares WHERE token = $1"
    )
    .bind(&token)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("Share not found".into()))?;

    // Check expiration
    if let Some(expires_at) = share.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(AppError::BadRequest("Share link expired".into()));
        }
    }

    // Get file info
    let file: crate::models::File = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1"
    )
    .bind(&share.file_id)
    .fetch_one(state.db.pool())
    .await?;

    Ok(Json(serde_json::json!({
        "token": share.token,
        "file": file,
        "permissions": share.permissions,
        "expires_at": share.expires_at,
        "has_password": share.password.is_some()
    })))
}