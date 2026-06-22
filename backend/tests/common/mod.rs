//! Shared test harness for the HTTP/Postgres integration tests under `tests/`.
//!
//! Pulled into each integration-test binary via `mod common;`. Drives the real
//! `build_router` against a real `PgPool` in-process via
//! `tower::ServiceExt::oneshot` (no port binding), plus request/assertion
//! helpers and entity factories reused across the per-domain test modules.

// Each integration-test binary pulls this module in via `mod common;`, but
// only uses the subset of helpers relevant to its own tests. Silence the
// dead-code warnings for helpers exercised solely by the *other* binary.
#![allow(dead_code)]

use std::path::PathBuf;

use axum::{Router, body::Body, http::Method, http::Request, http::StatusCode, http::header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::util::ServiceExt;
use tower_http::cors::AllowOrigin;
use uuid::Uuid;

use wrong_question_notebook_rs::{AppConfig, AppState, build_router};

pub struct TestApp {
    router: Router,
    pub pool: PgPool,
}

pub struct Session {
    pub id: Uuid,
    pub email: String,
    pub token: String,
}

impl TestApp {
    /// Build a `TestApp` from a pool provided by `#[sqlx::test]`, which has already
    /// created an isolated database and applied migrations. Each test thus runs
    /// against a fresh, parallel-safe database that is dropped when it finishes.
    pub async fn from_pool(pool: PgPool) -> Self {
        let state = AppState::new(test_config(), pool.clone());
        Self {
            router: build_router(state),
            pool,
        }
    }

    pub async fn sign_up(&self, label: &str, timezone: &str) -> Session {
        let email = format!("{label}-{}@example.test", Uuid::new_v4());
        let response = self
            .json(
                Method::POST,
                "/api/auth/sign-up",
                None,
                json!({
                    "email": email,
                    "password": "password123",
                    "timezone": timezone,
                }),
            )
            .await;
        response.assert_status(StatusCode::OK);
        Session {
            id: response.uuid_at(&["data", "user", "id"]),
            token: response.string_at(&["data", "access_token"]),
            email,
        }
    }

    pub async fn json(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Value,
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.send(
            builder
                .body(Body::from(body.to_string()))
                .expect("valid JSON request"),
        )
        .await
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> TestResponse {
        let mut builder = Request::builder().method(Method::GET).uri(uri);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.send(builder.body(Body::empty()).expect("valid GET request"))
            .await
    }

    /// GET that authenticates via the session cookie only (no `Authorization`
    /// header), exercising the cookie fallback in `extract_token`. The cookie
    /// name matches `test_config` (`wqn_test_session`).
    pub async fn get_with_cookie(&self, uri: &str, token: &str) -> TestResponse {
        let builder = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::COOKIE, format!("wqn_test_session={token}"));
        self.send(builder.body(Body::empty()).expect("valid GET request"))
            .await
    }

    pub async fn multipart(
        &self,
        uri: &str,
        token: Option<&str>,
        boundary: &str,
        body: String,
    ) -> TestResponse {
        let mut builder = Request::builder().method(Method::POST).uri(uri).header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        );
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.send(
            builder
                .body(Body::from(body))
                .expect("valid multipart request"),
        )
        .await
    }

    async fn send(&self, request: Request<Body>) -> TestResponse {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router response");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("read response body")
            .to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                panic!(
                    "response body is not JSON: {}",
                    String::from_utf8_lossy(&bytes)
                )
            })
        };
        TestResponse { status, body }
    }
}

pub struct TestResponse {
    pub status: StatusCode,
    pub body: Value,
}

impl TestResponse {
    pub fn assert_status(&self, expected: StatusCode) {
        assert_eq!(self.status, expected, "response body: {}", self.body);
    }

    pub fn assert_ok(&self) {
        self.assert_status(StatusCode::OK);
    }

    pub fn assert_created(&self) {
        self.assert_status(StatusCode::CREATED);
    }

    /// Assert both the status and that the error message contains `substr`.
    pub fn assert_error(&self, status: StatusCode, substr: &str) {
        assert_eq!(self.status, status, "response body: {}", self.body);
        let message = self
            .body
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("expected `error` field in: {}", self.body));
        assert!(
            message.contains(substr),
            "error message {message:?} does not contain {substr:?}; body: {}",
            self.body
        );
    }

    pub fn data(&self) -> &Value {
        self.value_at(&["data"])
    }

    pub fn string_at(&self, path: &[&str]) -> String {
        self.value_at(path)
            .as_str()
            .unwrap_or_else(|| panic!("expected string at {path:?}: {}", self.body))
            .to_owned()
    }

    pub fn uuid_at(&self, path: &[&str]) -> Uuid {
        Uuid::parse_str(&self.string_at(path))
            .unwrap_or_else(|err| panic!("expected UUID at {path:?}: {err}"))
    }

    pub fn value_at(&self, path: &[&str]) -> &Value {
        let mut value = &self.body;
        for segment in path {
            value = value
                .get(*segment)
                .unwrap_or_else(|| panic!("missing {segment} at {path:?}: {}", self.body));
        }
        value
    }
}

pub fn test_config() -> AppConfig {
    AppConfig {
        database_url: String::new(),
        database_max_connections: 5,
        bind_addr: "127.0.0.1:0".to_owned(),
        auth_jwt_secret: "integration-test-secret-integration-test-secret".to_owned(),
        auth_cookie_name: "wqn_test_session".to_owned(),
        auth_session_ttl_seconds: 3600,
        auth_cookie_secure: false,
        local_storage_root: test_storage_root(),
        local_storage_scan_command: Some("true".to_owned()),
        gemini_api_key: None,
        cron_secret: None,
        cors_origins: AllowOrigin::any(),
    }
}

fn test_storage_root() -> PathBuf {
    std::env::temp_dir().join(format!("wqn-http-db-test-{}", Uuid::new_v4()))
}

pub fn multipart_problem_upload(
    boundary: &str,
    problem_id: Uuid,
    file_name: &str,
    content_type: &str,
    bytes: &str,
) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"role\"\r\n\r\n\
         problem\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"problem_id\"\r\n\r\n\
         {problem_id}\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
         Content-Type: {content_type}\r\n\r\n\
         {bytes}\r\n\
         --{boundary}--\r\n"
    )
}

pub fn multipart_file(boundary: &str, file_name: &str, content_type: &str, bytes: &str) -> String {
    format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
         Content-Type: {content_type}\r\n\r\n\
         {bytes}\r\n\
         --{boundary}--\r\n"
    )
}

pub async fn promote_super_admin(app: &TestApp, user_id: Uuid) {
    sqlx::query("update user_profiles set user_role = 'super_admin' where id = $1")
        .bind(user_id)
        .execute(&app.pool)
        .await
        .expect("promote integration admin");
}

pub async fn create_subject(app: &TestApp, session: &Session) -> Uuid {
    let suffix = Uuid::new_v4().to_string();
    let response = app
        .json(
            Method::POST,
            "/api/subjects",
            Some(&session.token),
            json!({
                "name": format!("Subject {}", &suffix[..8]),
                "color": "#3366ff",
                "icon": "Calculator",
            }),
        )
        .await;
    response.assert_status(StatusCode::CREATED);
    response.uuid_at(&["data", "id"])
}

pub async fn create_tag(app: &TestApp, session: &Session, subject_id: Uuid, name: &str) -> Uuid {
    let response = app
        .json(
            Method::POST,
            "/api/tags",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "name": name,
            }),
        )
        .await;
    response.assert_status(StatusCode::CREATED);
    response.uuid_at(&["data", "id"])
}

pub async fn create_problem(app: &TestApp, session: &Session, subject_id: Uuid) -> Uuid {
    create_problem_with(app, session, subject_id, "short", "needs_review", Vec::new()).await
}

pub async fn create_problem_with(
    app: &TestApp,
    session: &Session,
    subject_id: Uuid,
    problem_type: &str,
    status: &str,
    tag_ids: Vec<Uuid>,
) -> Uuid {
    let response = app
        .json(
            Method::POST,
            "/api/problems",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "title": format!("Problem {}", Uuid::new_v4()),
                "problem_type": problem_type,
                "status": status,
                "tag_ids": tag_ids,
            }),
        )
        .await;
    response.assert_status(StatusCode::CREATED);
    response.uuid_at(&["data", "id"])
}
