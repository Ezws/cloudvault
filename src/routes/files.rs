use axum::{
    extract::{Extension, Path, State},
    http::header::CONTENT_TYPE,
    routing::{delete, get, patch, post},
    response::IntoResponse,
    Json, Router,
};
use std::path::PathBuf;

use crate::error::AppError;
use crate::models::{CreateFileRequest, File as FileModel, FileResponse, UpdateFileRequest};
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/files", get(list_files))
        .route("/api/files", post(create_file))
        .route("/api/files/upload", post(upload_file))
        .route("/api/files/{id}", get(get_file))
        .route("/api/files/{id}", patch(update_file))
        .route("/api/files/{id}", delete(delete_file))
        .route("/api/files/{id}/download", get(download_file))
}

async fn list_files(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
) -> Result<Json<Vec<FileResponse>>, AppError> {
    let files: Vec<FileModel> = sqlx::query_as(
        "SELECT * FROM files WHERE user_id = $1 ORDER BY is_folder DESC, name ASC"
    )
    .bind(&user_id)
    .fetch_all(state.db.pool())
    .await?;

    Ok(Json(files.into_iter().map(|f| f.into()).collect()))
}

async fn get_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<FileResponse>, AppError> {
    let file: FileModel = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2"
    )
    .bind(&id)
    .bind(&user_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    Ok(Json(file.into()))
}

async fn create_file(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
    axum::extract::Json(req): axum::extract::Json<CreateFileRequest>,
) -> Result<Json<FileResponse>, AppError> {
    if req.name.contains('/') || req.name.contains('\\') || req.name.contains("..") {
        return Err(AppError::BadRequest("Invalid file name".into()));
    }

    let parent_path = if let Some(ref parent_id) = req.parent_id {
        let parent: FileModel = sqlx::query_as(
            "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND is_folder = true"
        )
        .bind(parent_id)
        .bind(&user_id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("Parent folder not found".into()))?;
        parent.path.clone()
    } else {
        String::new()
    };

    let path = if parent_path.is_empty() {
        format!("/{}", req.name)
    } else {
        format!("{}/{}", parent_path, req.name)
    };

    let existing = sqlx::query(
        "SELECT id FROM files WHERE user_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND name = $3"
    )
    .bind(&user_id)
    .bind(&req.parent_id)
    .bind(&req.name)
    .fetch_optional(state.db.pool())
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("File or folder already exists".into()));
    }

    let file = FileModel::new(user_id, req.parent_id, &req.name, &path, req.is_folder);

    sqlx::query(
        r#"INSERT INTO files (id, user_id, parent_id, name, path, size, mime_type, is_folder, storage_type, storage_path, created_at, updated_at) 
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(&file.id)
    .bind(&file.user_id)
    .bind(&file.parent_id)
    .bind(&file.name)
    .bind(&file.path)
    .bind(file.size)
    .bind(&file.mime_type)
    .bind(file.is_folder)
    .bind(&file.storage_type)
    .bind(&file.storage_path)
    .bind(file.created_at)
    .bind(file.updated_at)
    .execute(state.db.pool())
    .await?;

    Ok(Json(file.into()))
}

/// Upload file - receives raw binary data with filename in query param
async fn upload_file(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: axum::body::Body,
) -> Result<Json<FileResponse>, AppError> {
    use tokio::io::AsyncWriteExt;

    let filename = params.get("filename")
        .cloned()
        .ok_or_else(|| AppError::BadRequest("Missing filename query parameter".into()))?;

    // Read body bytes using collect
    use http_body_util::BodyExt;
    let collected = body.collect().await;
    let body_bytes = match collected {
        Ok(b) => b.to_bytes(),
        Err(e) => return Err(AppError::BadRequest(e.to_string())),
    };

    // Save file to storage
    let storage_id = uuid::Uuid::new_v4().to_string();
    let storage_path = format!("{}/{}", &user_id[..8.min(user_id.len())], &storage_id);
    let full_path = PathBuf::from(&state.config.storage.local_path).join(&storage_path);
    
    // Create parent directory
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::Io(e))?;
    }
    
    let mut file = tokio::fs::File::create(&full_path).await.map_err(|e| AppError::Io(e))?;
    file.write_all(&body_bytes).await.map_err(|e| AppError::Io(e))?;

    // Detect MIME type
    let mime = mime_guess::from_path(&filename).first_or(mime_guess::mime::APPLICATION_OCTET_STREAM).to_string();

    // Create file record in DB
    let file_record = FileModel::new_with_storage(
        user_id,
        None,
        &filename,
        &format!("/{}", filename),
        false,
        body_bytes.len() as i64,
        &storage_path,
        Some(mime),
    );

    sqlx::query(
        r#"INSERT INTO files (id, user_id, parent_id, name, path, size, mime_type, is_folder, storage_type, storage_path, created_at, updated_at) 
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
    )
    .bind(&file_record.id)
    .bind(&file_record.user_id)
    .bind(&file_record.parent_id)
    .bind(&file_record.name)
    .bind(&file_record.path)
    .bind(file_record.size)
    .bind(&file_record.mime_type)
    .bind(file_record.is_folder)
    .bind(&file_record.storage_type)
    .bind(&file_record.storage_path)
    .bind(file_record.created_at)
    .bind(file_record.updated_at)
    .execute(state.db.pool())
    .await?;

    Ok(Json(file_record.into()))
}

/// Update file (rename or move)
async fn update_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(req): Json<UpdateFileRequest>,
) -> Result<Json<FileResponse>, AppError> {
    let file: FileModel = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2"
    )
    .bind(&id)
    .bind(&user_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    let new_name = req.name.unwrap_or_else(|| file.name.clone());
    
    // Determine the actual target parent_id
    // - If req.parent_id is None: keep current parent
    // - If req.parent_id is Some(""): move to root (null)
    // - If req.parent_id is Some(id): move to that folder
    let actual_parent_id: Option<String> = if let Some(ref pid) = req.parent_id {
        if pid.is_empty() {
            None  // move to root
        } else if pid == &file.id {
            return Err(AppError::BadRequest("Cannot move folder into itself".into()));
        } else {
            // Verify target folder exists
            let _: FileModel = sqlx::query_as(
                "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND is_folder = true"
            )
            .bind(pid)
            .bind(&user_id)
            .fetch_optional(state.db.pool())
            .await?
            .ok_or_else(|| AppError::NotFound("Target folder not found".into()))?;
            Some(pid.clone())
        }
    } else {
        file.parent_id.clone()  // keep current parent
    };

    // Validate new name
    if new_name.contains('/') || new_name.contains('\\') || new_name.contains("..") {
        return Err(AppError::BadRequest("Invalid file name".into()));
    }

    // Calculate new path based on actual parent
    let new_parent_path = if let Some(ref parent_id) = actual_parent_id {
        let parent_path: String = sqlx::query_scalar(
            "SELECT path FROM files WHERE id = $1"
        )
        .bind(parent_id)
        .fetch_one(state.db.pool())
        .await?;
        parent_path
    } else {
        String::new()  // root
    };

    let new_path = if new_parent_path.is_empty() {
        format!("/{}", new_name)
    } else if new_parent_path.ends_with('/') {
        format!("{}{}", new_parent_path, new_name)
    } else {
        format!("{}/{}", new_parent_path, new_name)
    };

    // Check for duplicate name in target folder
    let existing = sqlx::query(
        "SELECT id FROM files WHERE user_id = $1 AND id != $2 AND parent_id IS NOT DISTINCT FROM $3 AND name = $4"
    )
    .bind(&user_id)
    .bind(&id)
    .bind(&actual_parent_id)
    .bind(&new_name)
    .fetch_optional(state.db.pool())
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("File already exists in target folder".into()));
    }

    let now = chrono::Utc::now();


    sqlx::query(
        "UPDATE files SET name = $1, parent_id = $2, path = $3, updated_at = $4 WHERE id = $5"
    )
    .bind(&new_name)
    .bind(&actual_parent_id)
    .bind(&new_path)
    .bind(now)
    .bind(&id)
    .execute(state.db.pool())
    .await?;

    // Update paths of all children if this is a folder
    if file.is_folder && file.path != new_path {
        let old_path_prefix = format!("{}/", file.path);
        let new_path_prefix = format!("{}/", new_path);
        
        sqlx::query(
            "UPDATE files SET path = $1 || substr(path, $2) WHERE user_id = $3 AND path LIKE $4"
        )
        .bind(&new_path_prefix)
        .bind(old_path_prefix.len() as i32 + 1)
        .bind(&user_id)
        .bind(format!("{}%", old_path_prefix))
        .execute(state.db.pool())
        .await?;
    }

    // Fetch updated file
    let updated_file: FileModel = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1"
    )
    .bind(&id)
    .fetch_one(state.db.pool())
    .await?;

    Ok(Json(updated_file.into()))
}

/// Delete file or folder
async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let file: FileModel = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2"
    )
    .bind(&id)
    .bind(&user_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    // Delete storage file if not folder
    if !file.is_folder {
        if let Some(storage_path) = &file.storage_path {
            let full_path = PathBuf::from(&state.config.storage.local_path).join(storage_path);
            let _ = tokio::fs::remove_file(full_path).await;
        }
    }

    // Delete from database (CASCADE will handle children)
    sqlx::query("DELETE FROM files WHERE id = $1")
        .bind(&id)
        .execute(state.db.pool())
        .await?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// Download file
async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<impl IntoResponse, AppError> {
    let file: FileModel = sqlx::query_as(
        "SELECT * FROM files WHERE id = $1 AND user_id = $2"
    )
    .bind(&id)
    .bind(&user_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("File not found".into()))?;

    if file.is_folder {
        return Err(AppError::BadRequest("Cannot download a folder".into()));
    }

    let storage_path = file.storage_path
        .ok_or_else(|| AppError::Internal("Storage path not found".into()))?;
    
    let full_path = PathBuf::from(&state.config.storage.local_path).join(&storage_path);
    
    let file_handle = tokio::fs::File::open(&full_path).await
        .map_err(|e| AppError::Io(e))?;
    
    let stream = tokio_util::io::ReaderStream::new(file_handle);
    let body = axum::body::Body::from_stream(stream);
    
    let content_type = file.mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());
    
    Ok((
        [(CONTENT_TYPE, content_type)],
        body,
    ))
}