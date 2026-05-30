use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
    pub storage_quota: i64,
    pub storage_used: i64,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn new(username: String, password_hash: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            password_hash,
            email: None,
            storage_quota: 10 * 1024 * 1024 * 1024,
            storage_used: 0,
            is_admin: false,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub storage_quota: i64,
    pub storage_used: i64,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            storage_quota: user.storage_quota,
            storage_used: user.storage_used,
            is_admin: user.is_admin,
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct File {
    pub id: String,
    pub user_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub path: String,
    pub size: i64,
    pub mime_type: Option<String>,
    pub is_folder: bool,
    pub storage_type: String,
    pub storage_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl File {
    pub fn new(user_id: String, parent_id: Option<String>, name: &str, path: &str, is_folder: bool) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            parent_id,
            name: name.to_string(),
            path: path.to_string(),
            size: 0,
            mime_type: None,
            is_folder,
            storage_type: "local".to_string(),
            storage_path: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_with_storage(
        user_id: String,
        parent_id: Option<String>,
        name: &str,
        path: &str,
        is_folder: bool,
        size: i64,
        storage_path: &str,
        mime_type: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            parent_id,
            name: name.to_string(),
            path: path.to_string(),
            size,
            mime_type,
            is_folder,
            storage_type: "local".to_string(),
            storage_path: Some(storage_path.to_string()),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponse {
    pub id: String,
    pub user_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub path: String,
    pub size: i64,
    pub mime_type: Option<String>,
    pub is_folder: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<File> for FileResponse {
    fn from(file: File) -> Self {
        Self {
            id: file.id,
            user_id: file.user_id,
            parent_id: file.parent_id,
            name: file.name,
            path: file.path,
            size: file.size,
            mime_type: file.mime_type,
            is_folder: file.is_folder,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileRequest {
    pub name: String,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub is_folder: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFileRequest {
    pub name: Option<String>,   // null means keep current name
    pub parent_id: Option<String>,  // null/empty means root, actual ID means target folder
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Share {
    pub id: String,
    pub file_id: String,
    pub user_id: String,
    pub token: String,
    pub password: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub permissions: String,
    pub created_at: DateTime<Utc>,
}

impl Share {
    pub fn new(file_id: String, user_id: String, permissions: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            file_id,
            user_id,
            token: Uuid::new_v4().to_string().replace("-", ""),
            password: None,
            expires_at: None,
            permissions: permissions.to_string(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShareRequest {
    pub file_id: String,
    pub password: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default = "default_permissions")]
    pub permissions: String,
}

fn default_permissions() -> String {
    "read".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareResponse {
    pub id: String,
    pub file_id: String,
    pub token: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub permissions: String,
    pub created_at: DateTime<Utc>,
}

impl From<Share> for ShareResponse {
    fn from(share: Share) -> Self {
        Self {
            id: share.id,
            file_id: share.file_id,
            token: share.token,
            expires_at: share.expires_at,
            permissions: share.permissions,
            created_at: share.created_at,
        }
    }
}