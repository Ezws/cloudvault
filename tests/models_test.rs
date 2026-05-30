//! CloudVault Model Unit Tests

use cloudvault_server::models::{
    CreateFileRequest, CreateShareRequest, CreateUserRequest, File, Share, User, UserResponse,
};

mod common;

#[test]
fn test_user_new() {
    let user = User::new("testuser".to_string(), "hashed_password".to_string());
    
    assert_eq!(user.username, "testuser");
    assert_eq!(user.password_hash, "hashed_password");
    assert_eq!(user.storage_quota, 10 * 1024 * 1024 * 1024); // 10GB default
    assert_eq!(user.storage_used, 0);
    assert!(user.email.is_none());
}

#[test]
fn test_user_to_response() {
    let user = User::new("testuser".to_string(), "hashed_password".to_string());
    let response: UserResponse = user.into();
    
    assert_eq!(response.username, "testuser");
    // password_hash should not be in response
    assert!(response.storage_quota > 0);
}

#[test]
fn test_file_new() {
    let file = File::new(
        "user123".to_string(),
        None,
        "test.txt",
        "/test.txt",
        false,
    );
    
    assert_eq!(file.name, "test.txt");
    assert_eq!(file.path, "/test.txt");
    assert_eq!(file.user_id, "user123");
    assert!(!file.is_folder);
    assert!(file.parent_id.is_none());
    assert_eq!(file.size, 0);
    assert_eq!(file.storage_type, "local");
}

#[test]
fn test_file_new_with_storage() {
    let file = File::new_with_storage(
        "user123".to_string(),
        Some("parent123".to_string()),
        "document.pdf",
        "/Documents/document.pdf",
        false,
        1024,
        "uploads/abc123",
        Some("application/pdf".to_string()),
    );
    
    assert_eq!(file.name, "document.pdf");
    assert_eq!(file.path, "/Documents/document.pdf");
    assert_eq!(file.size, 1024);
    assert_eq!(file.storage_path, Some("uploads/abc123".to_string()));
    assert_eq!(file.mime_type, Some("application/pdf".to_string()));
}

#[test]
fn test_folder_new() {
    let folder = File::new(
        "user123".to_string(),
        None,
        "MyFolder",
        "/MyFolder",
        true,
    );
    
    assert_eq!(folder.name, "MyFolder");
    assert_eq!(folder.path, "/MyFolder");
    assert!(folder.is_folder);
    assert!(!folder.storage_path.is_some()); // folders don't have storage path
}

#[test]
fn test_share_new() {
    let share = Share::new(
        "file123".to_string(),
        "user123".to_string(),
        "read",
    );
    
    assert_eq!(share.file_id, "file123");
    assert_eq!(share.user_id, "user123");
    assert_eq!(share.permissions, "read");
    assert!(share.password.is_none());
    assert!(share.expires_at.is_none());
    assert!(!share.token.is_empty());
}

#[test]
fn test_share_permissions() {
    let share_read = Share::new("file1".to_string(), "user1".to_string(), "read");
    let share_write = Share::new("file2".to_string(), "user1".to_string(), "write");
    
    assert_eq!(share_read.permissions, "read");
    assert_eq!(share_write.permissions, "write");
}

#[test]
fn test_create_file_request_defaults() {
    let req = CreateFileRequest {
        name: "test.txt".to_string(),
        parent_id: None,
        is_folder: false,
    };
    
    assert_eq!(req.name, "test.txt");
    assert!(!req.is_folder);
}

#[test]
fn test_create_user_request() {
    let req = CreateUserRequest {
        username: "newuser".to_string(),
        password: "securepassword".to_string(),
        email: Some("user@example.com".to_string()),
    };
    
    assert_eq!(req.username, "newuser");
    assert_eq!(req.password, "securepassword");
    assert_eq!(req.email, Some("user@example.com".to_string()));
}

#[test]
fn test_create_share_request_defaults() {
    let req = CreateShareRequest {
        file_id: "file123".to_string(),
        password: None,
        expires_at: None,
        permissions: "read".to_string(),
    };
    
    assert_eq!(req.file_id, "file123");
    assert!(req.password.is_none());
    assert!(req.expires_at.is_none());
    assert_eq!(req.permissions, "read");
}