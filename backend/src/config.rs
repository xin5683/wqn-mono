use std::path::PathBuf;

use axum::http::{HeaderValue, Method};
use tower_http::cors::AllowOrigin;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub database_max_connections: u32,
    pub bind_addr: String,
    pub auth_jwt_secret: String,
    pub auth_cookie_name: String,
    pub auth_session_ttl_seconds: i64,
    pub auth_cookie_secure: bool,
    pub local_storage_root: PathBuf,
    pub local_storage_scan_command: Option<String>,
    pub gemini_api_key: Option<String>,
    pub cron_secret: Option<String>,
    pub cors_origins: AllowOrigin,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    #[error("invalid CORS_ALLOWED_ORIGINS: {0}")]
    InvalidCors(String),
    #[error("invalid DATABASE_MAX_CONNECTIONS: {0}")]
    InvalidPoolSize(String),
    #[error("invalid AUTH_SESSION_TTL_SECONDS: {0}")]
    InvalidSessionTtl(String),
    #[error("invalid AUTH_COOKIE_SECURE: {0}")]
    InvalidCookieSecure(String),
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = required("DATABASE_URL")?;
        let auth_jwt_secret = required("AUTH_JWT_SECRET")?;
        let auth_cookie_name =
            std::env::var("AUTH_COOKIE_NAME").unwrap_or_else(|_| "wqn_session".into());
        let auth_session_ttl_seconds = std::env::var("AUTH_SESSION_TTL_SECONDS")
            .unwrap_or_else(|_| "2592000".into())
            .parse()
            .map_err(|_| {
                ConfigError::InvalidSessionTtl(
                    std::env::var("AUTH_SESSION_TTL_SECONDS").unwrap_or_default(),
                )
            })?;
        let auth_cookie_secure = parse_bool_env("AUTH_COOKIE_SECURE", false)?;
        let local_storage_root = PathBuf::from(
            std::env::var("LOCAL_STORAGE_ROOT").unwrap_or_else(|_| "./storage".into()),
        );
        let bind_addr = std::env::var("APP_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
        let database_max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .map_err(|_| {
                ConfigError::InvalidPoolSize(
                    std::env::var("DATABASE_MAX_CONNECTIONS").unwrap_or_default(),
                )
            })?;

        Ok(Self {
            database_url,
            database_max_connections,
            bind_addr,
            auth_jwt_secret,
            auth_cookie_name,
            auth_session_ttl_seconds,
            auth_cookie_secure,
            local_storage_root,
            local_storage_scan_command: optional_non_empty("LOCAL_STORAGE_SCAN_COMMAND"),
            gemini_api_key: optional_non_empty("GEMINI_API_KEY"),
            cron_secret: optional_non_empty("CRON_SECRET"),
            cors_origins: parse_cors_origins()?,
        })
    }
}

pub fn allowed_methods() -> [Method; 6] {
    [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ]
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::Missing(name))
}

fn optional_non_empty(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_cors_origins() -> Result<AllowOrigin, ConfigError> {
    let raw = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_else(|_| "*".into());
    if raw.trim() == "*" || raw.trim().is_empty() {
        return Ok(AllowOrigin::any());
    }

    let values: Result<Vec<HeaderValue>, _> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(HeaderValue::from_str)
        .collect();

    values
        .map(AllowOrigin::list)
        .map_err(|_| ConfigError::InvalidCors(raw))
}

fn parse_bool_env(name: &'static str, default: bool) -> Result<bool, ConfigError> {
    let Ok(raw) = std::env::var(name) else {
        return Ok(default);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidCookieSecure(raw)),
    }
}
