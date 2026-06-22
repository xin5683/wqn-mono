use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Duration;

use crate::{response, state::AppState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RateLimitPolicy {
    name: &'static str,
    max_requests: usize,
    window: Duration,
}

const DEFAULT_POLICY: RateLimitPolicy = RateLimitPolicy {
    name: "api",
    max_requests: 300,
    window: Duration::from_secs(15 * 60),
};
const AUTH_POLICY: RateLimitPolicy = RateLimitPolicy {
    name: "auth",
    max_requests: 30,
    window: Duration::from_secs(15 * 60),
};
const FILE_UPLOAD_POLICY: RateLimitPolicy = RateLimitPolicy {
    name: "file_upload",
    max_requests: 20,
    window: Duration::from_secs(60 * 60),
};
const PROBLEM_CREATION_POLICY: RateLimitPolicy = RateLimitPolicy {
    name: "problem_creation",
    max_requests: 60,
    window: Duration::from_secs(60 * 60),
};

pub async fn security_headers(req: Request<Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    response
}

pub async fn rate_limit(State(state): State<AppState>, req: Request<Body>, next: Next) -> Response {
    if req.method() == axum::http::Method::OPTIONS {
        return next.run(req).await;
    }

    if is_suspicious_request(&req) {
        return response::error_response(StatusCode::BAD_REQUEST, "Invalid request", None)
            .into_response();
    }

    let policy = rate_limit_policy(req.method(), req.uri().path());
    let key = format!("{}:{}", policy.name, client_key(&req));

    if state
        .rate_limiter
        .check_with_limit(key, policy.max_requests, policy.window)
        .await
    {
        next.run(req).await
    } else {
        response::error_response(StatusCode::TOO_MANY_REQUESTS, "Too Many Requests", None)
            .into_response()
    }
}

fn client_key(req: &Request<Body>) -> String {
    req.headers()
        .get("x-forwarded-for")
        .or_else(|| req.headers().get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            req.headers()
                .get(header::USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "anonymous".to_owned())
}

fn rate_limit_policy(method: &Method, path: &str) -> RateLimitPolicy {
    // Apply the strict brute-force AUTH policy only to credential-submission
    // endpoints (login / sign-up / password change / forgot-password / logout
    // — all non-GET). GET /api/auth/me is a session-validation read that the
    // frontend's proxy fetches on every navigation from a single server-side
    // identity (no forwarded client IP → all such fetches share one key). If
    // /me sat under the tight 30/15min AUTH bucket, normal browsing exhausted
    // it and returned 429, which proxy.ts treated as "not logged in" and
    // bounced the user back to the login page. /me is not a brute-force
    // target, so it falls through to the looser DEFAULT policy.
    if path.starts_with("/api/auth/") && *method != Method::GET {
        AUTH_POLICY
    } else if *method == Method::POST
        && (path == "/api/files/upload"
            || path == "/api/profile/avatar"
            || path.starts_with("/api/qr-upload/"))
    {
        FILE_UPLOAD_POLICY
    } else if *method == Method::POST && path == "/api/problems" {
        PROBLEM_CREATION_POLICY
    } else {
        DEFAULT_POLICY
    }
}

fn is_suspicious_request(req: &Request<Body>) -> bool {
    let Some(path_and_query) = req.uri().path_and_query().map(|value| value.as_str()) else {
        return false;
    };
    let value = path_and_query.to_ascii_lowercase();
    [
        "../",
        "..%2f",
        "%2e%2e",
        "%00",
        "<script",
        "%3cscript",
        "union%20select",
        "union+select",
        "union select",
        "${",
        "$%7b",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Uri;

    #[test]
    fn classifies_rate_limit_policies() {
        assert_eq!(
            rate_limit_policy(&Method::POST, "/api/auth/login"),
            AUTH_POLICY
        );
        // The session-validation read (fetched by the frontend proxy on every
        // navigation) must NOT sit under the tight AUTH bucket — otherwise
        // browsing trips 429 and bounces users to the login page.
        assert_eq!(
            rate_limit_policy(&Method::GET, "/api/auth/me"),
            DEFAULT_POLICY
        );
        assert_eq!(
            rate_limit_policy(&Method::POST, "/api/files/upload"),
            FILE_UPLOAD_POLICY
        );
        assert_eq!(
            rate_limit_policy(&Method::POST, "/api/problems"),
            PROBLEM_CREATION_POLICY
        );
        assert_eq!(
            rate_limit_policy(&Method::GET, "/api/problems"),
            DEFAULT_POLICY
        );
    }

    #[test]
    fn detects_suspicious_path_and_query_patterns() {
        let req = Request::builder()
            .uri(Uri::from_static("/api/files/../secret"))
            .body(Body::empty())
            .expect("request");
        assert!(is_suspicious_request(&req));

        let req = Request::builder()
            .uri(Uri::from_static("/api/problems?q=%3Cscript%3E"))
            .body(Body::empty())
            .expect("request");
        assert!(is_suspicious_request(&req));

        let req = Request::builder()
            .uri(Uri::from_static("/api/problems?search_text=algebra"))
            .body(Body::empty())
            .expect("request");
        assert!(!is_suspicious_request(&req));
    }
}
