use axum::{
    extract::{Request, State},
    http::{StatusCode, Method},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub async fn jwt_auth_layer(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();

    // Allow preflight OPTIONS requests without auth
    if method == Method::OPTIONS {
        return next.run(request).await;
    }

    // Public endpoints that don't require authentication
    if path.starts_with("/api/auth/login")
        || path.starts_with("/api/auth/register")
        || path.starts_with("/api/shares/public")
        || path == "/health" {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body("Missing or invalid Authorization header".into())
                .unwrap()
        }
    };

    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.jwt.secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(token_data) => {
            request.extensions_mut().insert(token_data.claims.sub);
            next.run(request).await
        }
        Err(_) => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body("Invalid or expired token".into())
            .unwrap(),
    }
}