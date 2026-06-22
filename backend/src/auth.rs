use axum::{
    extract::{FromRef, FromRequestParts},
    http::{HeaderMap, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::AppError, services::accounts, state::AppState};

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OptionalAuthUser(pub Option<AuthUser>);

#[derive(Clone, Debug)]
pub struct AdminUser {
    pub user: AuthUser,
    pub role: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct LocalClaims {
    sub: String,
    email: String,
    exp: usize,
    iat: usize,
    token_version: i32,
}

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        authenticate_headers(&parts.headers, &state).await
    }
}

impl<S> FromRequestParts<S> for OptionalAuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let user = authenticate_headers(&parts.headers, &state).await.ok();
        Ok(Self(user))
    }
}

impl<S> FromRequestParts<S> for AdminUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let user = authenticate_headers(&parts.headers, &state).await?;
        let role = role_for_user(&state.pool, user.id).await?;
        if accounts::is_admin_role(&role) {
            Ok(Self { user, role })
        } else {
            Err(AppError::Forbidden)
        }
    }
}

impl IntoResponse for OptionalAuthUser {
    fn into_response(self) -> Response {
        StatusCode::NO_CONTENT.into_response()
    }
}

pub async fn role_for_user(pool: &PgPool, user_id: Uuid) -> Result<String, AppError> {
    accounts::role_for_user(pool, user_id).await
}

pub async fn authenticate_headers(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<AuthUser, AppError> {
    let token =
        extract_token(headers, &state.config.auth_cookie_name).ok_or(AppError::Unauthorized)?;
    authenticate_token(&state.pool, &state.config.auth_jwt_secret, &token).await
}

pub async fn authenticate_token(
    pool: &PgPool,
    jwt_secret: &str,
    token: &str,
) -> Result<AuthUser, AppError> {
    let claims = decode_token(jwt_secret, token)?;
    let id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
    let row = sqlx::query_as::<_, (String, i32, bool)>(
        "select email, token_version, disabled_at is not null from app_users where id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    if row.2 || row.1 != claims.token_version {
        return Err(AppError::Unauthorized);
    }

    Ok(AuthUser {
        id,
        email: Some(row.0),
    })
}

pub fn issue_token(
    user_id: Uuid,
    email: &str,
    token_version: i32,
    jwt_secret: &str,
    ttl_seconds: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let expires_at = now + Duration::seconds(ttl_seconds);
    let claims = LocalClaims {
        sub: user_id.to_string(),
        email: email.to_owned(),
        iat: now.timestamp() as usize,
        exp: expires_at.timestamp() as usize,
        token_version,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|err| AppError::Internal(format!("Failed to issue token: {err}")))
}

pub fn session_cookie_value(name: &str, token: &str, ttl_seconds: i64, secure: bool) -> String {
    let mut cookie = format!(
        "{name}={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
        ttl_seconds.max(0)
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn expired_session_cookie_value(name: &str, secure: bool) -> String {
    let mut cookie = format!(
        "{name}=; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; HttpOnly; SameSite=Lax"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn extract_token(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| extract_cookie_token(headers, cookie_name))
}

fn decode_token(jwt_secret: &str, token: &str) -> Result<LocalClaims, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;

    decode::<LocalClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

fn extract_cookie_token(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for raw_cookie in cookie_header.split(';') {
        let Some((name, value)) = raw_cookie.trim().split_once('=') else {
            continue;
        };
        if name.trim() == cookie_name {
            let decoded = urlencoding::decode(value.trim()).ok()?.into_owned();
            return (!decoded.is_empty()).then_some(decoded);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, header};

    use super::{expired_session_cookie_value, extract_token, session_cookie_value};

    #[test]
    fn extracts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer abc.def.ghi".parse().unwrap());
        assert_eq!(
            extract_token(&headers, "wqn_session").as_deref(),
            Some("abc.def.ghi")
        );
    }

    #[test]
    fn extracts_configured_cookie_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "other=1; wqn_session=abc.def.ghi; theme=dark"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            extract_token(&headers, "wqn_session").as_deref(),
            Some("abc.def.ghi")
        );
    }

    #[test]
    fn bearer_token_takes_precedence_over_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            "Bearer bearer-token".parse().unwrap(),
        );
        headers.insert(header::COOKIE, "wqn_session=cookie-token".parse().unwrap());

        assert_eq!(
            extract_token(&headers, "wqn_session").as_deref(),
            Some("bearer-token")
        );
    }

    #[test]
    fn decodes_cookie_token_and_ignores_empty_values() {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, "wqn_session=abc%2Edef".parse().unwrap());
        assert_eq!(
            extract_token(&headers, "wqn_session").as_deref(),
            Some("abc.def")
        );

        let mut empty = HeaderMap::new();
        empty.insert(header::COOKIE, "wqn_session=; other=1".parse().unwrap());
        assert_eq!(extract_token(&empty, "wqn_session"), None);
    }

    #[test]
    fn builds_session_cookie_values() {
        let cookie = session_cookie_value("wqn_session", "token", 60, true);
        assert!(cookie.contains("wqn_session=token"));
        assert!(cookie.contains("Max-Age=60"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));

        let expired = expired_session_cookie_value("wqn_session", false);
        assert!(expired.contains("wqn_session="));
        assert!(expired.contains("Max-Age=0"));
        assert!(expired.contains("Expires=Thu, 01 Jan 1970 00:00:00 GMT"));
        assert!(!expired.contains("Secure"));
    }
}
