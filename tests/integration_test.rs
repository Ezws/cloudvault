//! CloudVault Integration Tests

use cloudvault_server::models::{CreateFileRequest, CreateShareRequest};

mod common;

#[tokio::test]
async fn test_auth_register_and_login() -> Result<(), Box<dyn std::error::Error>> {
    let mut setup = common::TestSetup::new().await?;
    
    let username = format!("test_user_{}", uuid::Uuid::new_v4());
    
    // Register
    let response = reqwest::Client::new()
        .post("http://localhost:8080/api/auth/register")
        .json(&serde_json::json!({
            "username": username,
            "password": "testpass123"
        }))
        .send()
        .await?;
    
    assert!(response.status().is_success());
    
    let body: serde_json::Value = response.json().await?;
    assert_eq!(body["username"], username);
    let user_id = body["id"].as_str().unwrap();
    
    // Login
    let login_response = reqwest::Client::new()
        .post("http://localhost:8080/api/auth/login")
        .json(&serde_json::json!({
            "username": username,
            "password": "testpass123"
        }))
        .send()
        .await?;
    
    assert!(login_response.status().is_success());
    
    let login_body: serde_json::Value = login_response.json().await?;
    assert!(login_body["token"].is_string());
    let token = login_body["token"].as_str().unwrap();
    
    // Verify token works
    let me_response = reqwest::Client::new()
        .get("http://localhost:8080/api/auth/me")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;
    
    assert!(me_response.status().is_success());
    
    // Cleanup
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(setup.state.db.pool())
        .await?;
    
    Ok(())
}

#[tokio::test]
async fn test_file_crud() -> Result<(), Box<dyn std::error::Error>> {
    let mut setup = common::TestSetup::new().await?;
    setup.create_user("testuser", "testpass").await?;
    
    let client = reqwest::Client::new();
    let token = format!("Bearer {}", setup.token);
    
    // Create folder
    let create_response = client
        .post("http://localhost:8080/api/files")
        .header("Authorization", &token)
        .json(&CreateFileRequest {
            name: "TestFolder".to_string(),
            parent_id: None,
            is_folder: true,
        })
        .send()
        .await?;
    
    assert!(create_response.status().is_success());
    let folder: serde_json::Value = create_response.json().await?;
    let folder_id = folder["id"].as_str().unwrap();
    assert_eq!(folder["name"], "TestFolder");
    assert!(folder["is_folder"].as_bool().unwrap());
    
    // List files
    let list_response = client
        .get("http://localhost:8080/api/files")
        .header("Authorization", &token)
        .send()
        .await?;
    
    assert!(list_response.status().is_success());
    
    // Get single file
    let get_response = client
        .get(&format!("http://localhost:8080/api/files/{}", folder_id))
        .header("Authorization", &token)
        .send()
        .await?;
    
    assert!(get_response.status().is_success());
    
    // Rename
    let rename_response = client
        .patch(&format!("http://localhost:8080/api/files/{}", folder_id))
        .header("Authorization", &token)
        .json(&serde_json::json!({"name": "RenamedFolder"}))
        .send()
        .await?;
    
    assert!(rename_response.status().is_success());
    let renamed: serde_json::Value = rename_response.json().await?;
    assert_eq!(renamed["name"], "RenamedFolder");
    
    // Delete
    let delete_response = client
        .delete(&format!("http://localhost:8080/api/files/{}", folder_id))
        .header("Authorization", &token)
        .send()
        .await?;
    
    assert!(delete_response.status().is_success());
    
    // Verify deleted
    let verify_response = client
        .get(&format!("http://localhost:8080/api/files/{}", folder_id))
        .header("Authorization", &token)
        .send()
        .await?;
    
    assert_eq!(verify_response.status().as_u16(), 404);
    
    // Cleanup
    setup.cleanup().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_file_upload() -> Result<(), Box<dyn std::error::Error>> {
    let mut setup = common::TestSetup::new().await?;
    let username = format!("upload_user_{}", uuid::Uuid::new_v4());
    setup.create_user(&username, "testpass").await?;
    
    let client = reqwest::Client::new();
    let token = format!("Bearer {}", setup.token);
    
    let upload_response = client
        .post("http://localhost:8080/api/files/upload?filename=test.txt")
        .header("Authorization", &token)
        .header("Content-Type", "text/plain")
        .body("Hello, World!")
        .send()
        .await?;
    
    assert!(upload_response.status().is_success());
    
    let uploaded: serde_json::Value = upload_response.json().await?;
    assert_eq!(uploaded["name"], "test.txt");
    assert!(uploaded["size"].as_i64().unwrap() > 0);
    
    // Cleanup
    setup.cleanup().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_share_operations() -> Result<(), Box<dyn std::error::Error>> {
    let mut setup = common::TestSetup::new().await?;
    let username = format!("share_user_{}", uuid::Uuid::new_v4());
    setup.create_user(&username, "testpass").await?;
    
    let client = reqwest::Client::new();
    let token = format!("Bearer {}", setup.token);
    
    // Create a file
    let file_response = client
        .post("http://localhost:8080/api/files")
        .header("Authorization", &token)
        .json(&CreateFileRequest {
            name: "SharedFile".to_string(),
            parent_id: None,
            is_folder: true,
        })
        .send()
        .await?;
    
    let file: serde_json::Value = file_response.json().await?;
    let file_id = file["id"].as_str().unwrap();
    
    // Create share
    let share_response = client
        .post("http://localhost:8080/api/shares")
        .header("Authorization", &token)
        .json(&CreateShareRequest {
            file_id: file_id.to_string(),
            password: None,
            expires_at: None,
            permissions: "read".to_string(),
        })
        .send()
        .await?;
    
    assert!(share_response.status().is_success());
    let share: serde_json::Value = share_response.json().await?;
    let share_token = share["token"].as_str().unwrap();
    
    // Access public share
    let public_response = client
        .get(&format!("http://localhost:8080/api/shares/public/{}", share_token))
        .send()
        .await?;
    
    assert!(public_response.status().is_success());
    
    // List shares
    let list_response = client
        .get("http://localhost:8080/api/shares")
        .header("Authorization", &token)
        .send()
        .await?;
    
    assert!(list_response.status().is_success());
    
    // Cleanup
    setup.cleanup().await?;
    
    Ok(())
}

#[tokio::test]
async fn test_unauthorized_access() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Try to list files without auth
    let response = client
        .get("http://localhost:8080/api/files")
        .send()
        .await?;
    
    assert_eq!(response.status().as_u16(), 401);
    
    // Try to create share without auth
    let share_response = client
        .post("http://localhost:8080/api/shares")
        .json(&serde_json::json!({
            "file_id": "some-file-id",
            "permissions": "read"
        }))
        .send()
        .await?;
    
    assert_eq!(share_response.status().as_u16(), 401);
    
    Ok(())
}

#[tokio::test]
async fn test_health_endpoint() -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::get("http://localhost:8080/health").await?;
    assert!(response.status().is_success());
    
    let body: String = response.text().await?;
    assert_eq!(body, "OK");
    
    Ok(())
}