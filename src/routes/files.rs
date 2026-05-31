use axum::{
    body::Body,
    extract::{Extension, Path, Query, State},
    http::{
        header::{
            ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
        },
        HeaderMap, StatusCode,
    },
    routing::{delete, get, patch, post},
    response::Response,
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
        .route("/api/files/uploads/init", post(init_resumable_upload))
        .route("/api/files/uploads/{upload_id}/status", get(get_upload_status))
        .route("/api/files/uploads/{upload_id}/chunk", post(upload_chunk))
        .route("/api/files/uploads/{upload_id}/complete", post(complete_upload))
        .route("/api/files/{id}", get(get_file))
        .route("/api/files/{id}", patch(update_file))
        .route("/api/files/{id}", delete(delete_file))
        .route("/api/files/{id}/download", get(download_file))
}

#[derive(Debug, serde::Deserialize)]
struct InitUploadRequest {
    filename: String,
    parent_id: Option<String>,
    size: i64,
}

#[derive(Debug, serde::Serialize)]
struct InitUploadResponse {
    upload_id: String,
    uploaded_bytes: u64,
}

#[derive(Debug, serde::Serialize)]
struct UploadStatusResponse {
    upload_id: String,
    uploaded_bytes: u64,
}

#[derive(Debug, serde::Deserialize)]
struct CompleteUploadRequest {
    filename: String,
    parent_id: Option<String>,
    size: i64,
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

    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(AppError::BadRequest("Invalid file name".into()));
    }

    let parent_id = params.get("parent_id").filter(|id| !id.is_empty()).cloned();
    let parent_path = if let Some(ref parent_id) = parent_id {
        let parent: FileModel = sqlx::query_as(
            "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND is_folder = true"
        )
        .bind(parent_id)
        .bind(&user_id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("Parent folder not found".into()))?;
        parent.path
    } else {
        String::new()
    };

    let existing = sqlx::query(
        "SELECT id FROM files WHERE user_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND name = $3"
    )
    .bind(&user_id)
    .bind(&parent_id)
    .bind(&filename)
    .fetch_optional(state.db.pool())
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("File already exists".into()));
    }

    let file_path = if parent_path.is_empty() {
        format!("/{}", filename)
    } else {
        format!("{}/{}", parent_path, filename)
    };

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
        parent_id,
        &filename,
        &file_path,
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

async fn init_resumable_upload(
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
    Json(req): Json<InitUploadRequest>,
) -> Result<Json<InitUploadResponse>, AppError> {
    validate_filename(&req.filename)?;
    validate_parent_folder(&state, &user_id, req.parent_id.as_ref()).await?;

    if req.size < 0 {
        return Err(AppError::BadRequest("Invalid file size".into()));
    }

    let upload_id = uuid::Uuid::new_v4().to_string();
    let temp_path = upload_temp_path(&state, &user_id, &upload_id);

    if let Some(parent) = temp_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(AppError::Io)?;
    }

    tokio::fs::File::create(&temp_path).await.map_err(AppError::Io)?;

    Ok(Json(InitUploadResponse {
        upload_id,
        uploaded_bytes: 0,
    }))
}

async fn get_upload_status(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<UploadStatusResponse>, AppError> {
    let temp_path = upload_temp_path(&state, &user_id, &upload_id);
    let uploaded_bytes = match tokio::fs::metadata(&temp_path).await {
        Ok(metadata) => metadata.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(AppError::Io(e)),
    };

    Ok(Json(UploadStatusResponse {
        upload_id,
        uploaded_bytes,
    }))
}

async fn upload_chunk(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
    Extension(user_id): Extension<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    body: Body,
) -> Result<Json<UploadStatusResponse>, AppError> {
    use http_body_util::BodyExt;
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};

    let offset = params
        .get("offset")
        .ok_or_else(|| AppError::BadRequest("Missing offset query parameter".into()))?
        .parse::<u64>()
        .map_err(|_| AppError::BadRequest("Invalid offset".into()))?;

    let temp_path = upload_temp_path(&state, &user_id, &upload_id);
    if let Some(parent) = temp_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(AppError::Io)?;
    }

    let current_len = match tokio::fs::metadata(&temp_path).await {
        Ok(metadata) => metadata.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(AppError::Io(e)),
    };

    if offset > current_len {
        return Err(AppError::BadRequest("Chunk offset is beyond uploaded bytes".into()));
    }

    let body_bytes = body
        .collect()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .to_bytes();

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(&temp_path)
        .await
        .map_err(AppError::Io)?;

    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(AppError::Io)?;
    file.write_all(&body_bytes).await.map_err(AppError::Io)?;
    file.flush().await.map_err(AppError::Io)?;

    let uploaded_bytes = offset + body_bytes.len() as u64;

    Ok(Json(UploadStatusResponse {
        upload_id,
        uploaded_bytes,
    }))
}

async fn complete_upload(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(req): Json<CompleteUploadRequest>,
) -> Result<Json<FileResponse>, AppError> {
    validate_filename(&req.filename)?;

    let parent_id = req.parent_id.filter(|id| !id.is_empty());
    let parent_path = validate_parent_folder(&state, &user_id, parent_id.as_ref()).await?;
    let file_path = build_display_path(&parent_path, &req.filename);

    let temp_path = upload_temp_path(&state, &user_id, &upload_id);
    let uploaded_size = tokio::fs::metadata(&temp_path).await.map_err(AppError::Io)?.len() as i64;

    if uploaded_size != req.size {
        return Err(AppError::BadRequest("Uploaded size does not match expected file size".into()));
    }

    ensure_no_duplicate(&state, &user_id, &parent_id, &req.filename).await?;

    let storage_id = uuid::Uuid::new_v4().to_string();
    let storage_path = format!("{}/{}", &user_id[..8.min(user_id.len())], &storage_id);
    let full_path = PathBuf::from(&state.config.storage.local_path).join(&storage_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(AppError::Io)?;
    }

    tokio::fs::rename(&temp_path, &full_path).await.map_err(AppError::Io)?;

    let mime = mime_guess::from_path(&req.filename)
        .first_or(mime_guess::mime::APPLICATION_OCTET_STREAM)
        .to_string();

    let file_record = FileModel::new_with_storage(
        user_id,
        parent_id,
        &req.filename,
        &file_path,
        false,
        uploaded_size,
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
    headers: HeaderMap,
) -> Result<Response, AppError> {
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
    let total_size = tokio::fs::metadata(&full_path).await.map_err(AppError::Io)?.len();
    let range = if total_size == 0 {
        None
    } else {
        parse_range(
            headers.get(RANGE).and_then(|value| value.to_str().ok()),
            total_size,
        )?
    };
    let (start, end, status) = if let Some((start, end)) = range {
        (start, end, StatusCode::PARTIAL_CONTENT)
    } else if total_size == 0 {
        (0, 0, StatusCode::OK)
    } else {
        (0, total_size - 1, StatusCode::OK)
    };
    let content_length = if total_size == 0 { 0 } else { end - start + 1 };
    
    let mut file_handle = tokio::fs::File::open(&full_path).await.map_err(AppError::Io)?;
    if start > 0 {
        use tokio::io::AsyncSeekExt;
        file_handle
            .seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(AppError::Io)?;
    }
    
    use tokio::io::AsyncReadExt;
    let stream = tokio_util::io::ReaderStream::new(file_handle.take(content_length));
    let body = Body::from_stream(stream);
    
    let content_type = file.mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let content_disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        sanitize_ascii_filename(&file.name),
        percent_encode_filename(&file.name),
    );

    let mut builder = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_DISPOSITION, content_disposition)
        .header(ACCEPT_RANGES, "bytes")
        .header(CONTENT_LENGTH, content_length.to_string());

    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, total_size),
        );
    }

    builder
        .body(body)
        .map_err(|e| AppError::Internal(e.to_string()))
}

fn sanitize_ascii_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|ch| match ch {
            '"' | '\\' | '\r' | '\n' => '_',
            ch if ch.is_ascii() && !ch.is_control() => ch,
            _ => '_',
        })
        .collect();

    if sanitized.is_empty() {
        "download".to_string()
    } else {
        sanitized
    }
}

fn percent_encode_filename(name: &str) -> String {
    let mut encoded = String::new();

    for byte in name.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => {
                encoded.push(*byte as char);
            }
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }

    encoded
}

fn validate_filename(filename: &str) -> Result<(), AppError> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        return Err(AppError::BadRequest("Invalid file name".into()));
    }

    Ok(())
}

async fn validate_parent_folder(
    state: &AppState,
    user_id: &str,
    parent_id: Option<&String>,
) -> Result<String, AppError> {
    if let Some(parent_id) = parent_id {
        let parent: FileModel = sqlx::query_as(
            "SELECT * FROM files WHERE id = $1 AND user_id = $2 AND is_folder = true",
        )
        .bind(parent_id)
        .bind(user_id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("Parent folder not found".into()))?;
        Ok(parent.path)
    } else {
        Ok(String::new())
    }
}

async fn ensure_no_duplicate(
    state: &AppState,
    user_id: &str,
    parent_id: &Option<String>,
    filename: &str,
) -> Result<(), AppError> {
    let existing = sqlx::query(
        "SELECT id FROM files WHERE user_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND name = $3",
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(filename)
    .fetch_optional(state.db.pool())
    .await?;

    if existing.is_some() {
        Err(AppError::Conflict("File already exists".into()))
    } else {
        Ok(())
    }
}

fn build_display_path(parent_path: &str, filename: &str) -> String {
    if parent_path.is_empty() {
        format!("/{}", filename)
    } else {
        format!("{}/{}", parent_path, filename)
    }
}

fn upload_temp_path(state: &AppState, user_id: &str, upload_id: &str) -> PathBuf {
    PathBuf::from(&state.config.storage.local_path)
        .join(".uploads")
        .join(&user_id[..8.min(user_id.len())])
        .join(upload_id)
}

fn parse_range(range: Option<&str>, total_size: u64) -> Result<Option<(u64, u64)>, AppError> {
    let Some(range) = range else {
        return Ok(None);
    };

    let Some(spec) = range.strip_prefix("bytes=") else {
        return Err(AppError::BadRequest("Invalid Range header".into()));
    };

    let (start_raw, end_raw) = spec
        .split_once('-')
        .ok_or_else(|| AppError::BadRequest("Invalid Range header".into()))?;

    if total_size == 0 {
        return Ok(Some((0, 0)));
    }

    let (start, end) = if start_raw.is_empty() {
        let suffix = end_raw
            .parse::<u64>()
            .map_err(|_| AppError::BadRequest("Invalid Range header".into()))?;
        if suffix == 0 {
            return Err(AppError::BadRequest("Invalid Range header".into()));
        }
        let start = total_size.saturating_sub(suffix);
        (start, total_size - 1)
    } else {
        let start = start_raw
            .parse::<u64>()
            .map_err(|_| AppError::BadRequest("Invalid Range header".into()))?;
        let end = if end_raw.is_empty() {
            total_size - 1
        } else {
            end_raw
                .parse::<u64>()
                .map_err(|_| AppError::BadRequest("Invalid Range header".into()))?
        };
        (start, end)
    };

    if start >= total_size || end < start {
        return Err(AppError::BadRequest("Range is not satisfiable".into()));
    }

    Ok(Some((start, end.min(total_size - 1))))
}
