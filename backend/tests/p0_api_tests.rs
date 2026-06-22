//! P0 API-correctness tests for the core spine: authentication, per-user
//! ownership isolation, content-limit enforcement, profile uniqueness, and
//! admin access control. These exercise the real router + real Postgres via
//! `#[sqlx::test]` (each test gets an isolated database).

use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;

mod common;
use common::*;

// ---------------------------------------------------------------------------
// G1 — Auth
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn sign_up_returns_token_and_user(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let email = format!("signup-{}@example.test", uuid::Uuid::new_v4());

    let response = app
        .json(
            Method::POST,
            "/api/auth/sign-up",
            None,
            json!({ "email": email, "password": "password123", "timezone": "UTC" }),
        )
        .await;

    response.assert_status(StatusCode::OK);
    assert_eq!(response.value_at(&["data", "user", "email"]).as_str(), Some(email.as_str()));
    assert!(response
        .value_at(&["data", "access_token"])
        .as_str()
        .is_some_and(|t| !t.is_empty()));
    assert_eq!(response.value_at(&["data", "token_type"]).as_str(), Some("bearer"));
}

#[sqlx::test]
async fn sign_up_rejects_duplicate_email(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let email = format!("dup-{}@example.test", uuid::Uuid::new_v4());

    app.json(
        Method::POST,
        "/api/auth/sign-up",
        None,
        json!({ "email": email, "password": "password123", "timezone": "UTC" }),
    )
    .await
    .assert_status(StatusCode::OK);

    app.json(
        Method::POST,
        "/api/auth/sign-up",
        None,
        json!({ "email": email, "password": "password123", "timezone": "UTC" }),
    )
    .await
    .assert_error(StatusCode::CONFLICT, "already registered");
}

#[sqlx::test]
async fn sign_up_rejects_invalid_email_and_short_password(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;

    app.json(
        Method::POST,
        "/api/auth/sign-up",
        None,
        json!({ "email": "not-an-email", "password": "password123", "timezone": "UTC" }),
    )
    .await
    .assert_error(StatusCode::BAD_REQUEST, "Invalid email");

    let email = format!("shortpw-{}@example.test", uuid::Uuid::new_v4());
    app.json(
        Method::POST,
        "/api/auth/sign-up",
        None,
        json!({ "email": email, "password": "short", "timezone": "UTC" }),
    )
    .await
    .assert_error(StatusCode::BAD_REQUEST, "at least 8");
}

#[sqlx::test]
async fn login_succeeds_or_rejects_based_on_password(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("login", "UTC").await;

    app.json(
        Method::POST,
        "/api/auth/login",
        None,
        json!({ "email": session.email, "password": "password123" }),
    )
    .await
    .assert_status(StatusCode::OK);

    app.json(
        Method::POST,
        "/api/auth/login",
        None,
        json!({ "email": session.email, "password": "wrong-password" }),
    )
    .await
    .assert_status(StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn me_requires_a_valid_token(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;

    app.get("/api/auth/me", None).await.assert_status(StatusCode::UNAUTHORIZED);
    app.get("/api/auth/me", Some("not.a.real.token"))
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn me_returns_the_authenticated_user(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("me", "UTC").await;

    let response = app.get("/api/auth/me", Some(&session.token)).await;
    response.assert_status(StatusCode::OK);
    assert_eq!(
        response.value_at(&["data", "email"]).as_str(),
        Some(session.email.as_str())
    );
    // A freshly signed-up user has the default `user` role and is not an admin.
    assert_eq!(response.value_at(&["data", "role"]).as_str(), Some("user"));
    assert_eq!(response.value_at(&["data", "is_admin"]).as_bool(), Some(false));
}

#[sqlx::test]
async fn me_accepts_session_cookie(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("cookie", "UTC").await;

    // No Authorization header — authenticate purely via the session cookie.
    app.get_with_cookie("/api/auth/me", &session.token)
        .await
        .assert_status(StatusCode::OK);
}

#[sqlx::test]
async fn change_password_invalidates_previous_token(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("change-pw", "UTC").await;
    let old_token = session.token.clone();

    app.json(
        Method::PATCH,
        "/api/auth/password",
        Some(&old_token),
        json!({ "current_password": "password123", "new_password": "newpassword123" }),
    )
    .await
    .assert_status(StatusCode::OK);

    // The old token's `token_version` is now stale.
    app.get("/api/auth/me", Some(&old_token))
        .await
        .assert_status(StatusCode::UNAUTHORIZED);

    // Logging in with the new password works and yields a fresh, valid token.
    let new_session = app
        .json(
            Method::POST,
            "/api/auth/login",
            None,
            json!({ "email": session.email, "password": "newpassword123" }),
        )
        .await;
    new_session.assert_status(StatusCode::OK);
    let new_token = new_session.string_at(&["data", "access_token"]);
    app.get("/api/auth/me", Some(&new_token))
        .await
        .assert_status(StatusCode::OK);
}

#[sqlx::test]
async fn forgot_password_reports_unconfigured(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    app.json(
        Method::POST,
        "/api/auth/forgot-password",
        None,
        json!({ "email": "anyone@example.test" }),
    )
    .await
    .assert_error(StatusCode::INTERNAL_SERVER_ERROR, "not configured");
}

// ---------------------------------------------------------------------------
// G2/G3 — Per-user ownership isolation (404, not 403)
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn subject_access_is_isolated_per_user(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let owner = app.sign_up("subject-owner", "UTC").await;
    let other = app.sign_up("subject-other", "UTC").await;

    let subject_id = create_subject(&app, &owner).await;

    // Another user cannot read, rename, or delete the owner's subject.
    app.get(&format!("/api/subjects/{subject_id}"), Some(&other.token))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.json(
        Method::PATCH,
        &format!("/api/subjects/{subject_id}"),
        Some(&other.token),
        json!({ "color": "#000000" }),
    )
    .await
    .assert_status(StatusCode::NOT_FOUND);
    app.json(
        Method::DELETE,
        &format!("/api/subjects/{subject_id}"),
        Some(&other.token),
        json!({}),
    )
    .await
    .assert_status(StatusCode::NOT_FOUND);

    // The owner still sees and owns it.
    app.get(&format!("/api/subjects/{subject_id}"), Some(&owner.token))
        .await
        .assert_status(StatusCode::OK);
}

#[sqlx::test]
async fn problem_access_is_isolated_per_user(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let owner = app.sign_up("problem-owner", "UTC").await;
    let other = app.sign_up("problem-other", "UTC").await;

    let subject_id = create_subject(&app, &owner).await;
    let problem_id = create_problem(&app, &owner, subject_id).await;

    app.get(&format!("/api/problems/{problem_id}"), Some(&other.token))
        .await
        .assert_status(StatusCode::NOT_FOUND);
    app.json(
        Method::DELETE,
        &format!("/api/problems/{problem_id}"),
        Some(&other.token),
        json!({}),
    )
    .await
    .assert_status(StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn tag_access_is_isolated_per_user(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let owner = app.sign_up("tag-owner", "UTC").await;
    let other = app.sign_up("tag-other", "UTC").await;

    let subject_id = create_subject(&app, &owner).await;
    let tag_id = create_tag(&app, &owner, subject_id, "owner-tag").await;

    app.json(
        Method::PATCH,
        &format!("/api/tags/{tag_id}"),
        Some(&other.token),
        json!({ "name": "stolen" }),
    )
    .await
    .assert_status(StatusCode::NOT_FOUND);
    app.json(
        Method::DELETE,
        &format!("/api/tags/{tag_id}"),
        Some(&other.token),
        json!({}),
    )
    .await
    .assert_status(StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// G2 — Content-limit enforcement
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn subject_creation_enforces_content_limit(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("limit", "UTC").await;

    // Cap this user to a single subject so we can exhaust it quickly.
    sqlx::query(
        "insert into content_limit_overrides (user_id, resource_type, limit_value) \
         values ($1, 'subjects', 1) \
         on conflict (user_id, resource_type) \
         do update set limit_value = excluded.limit_value",
    )
    .bind(session.id)
    .execute(&app.pool)
    .await
    .expect("set subjects override");

    create_subject(&app, &session).await; // 1st subject: current 0 < 1, allowed → 201

    app.json(
        Method::POST,
        "/api/subjects",
        Some(&session.token),
        json!({ "name": "Second subject", "color": "#112233", "icon": "Book" }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// G5 — Profile username uniqueness + onboarding
// ---------------------------------------------------------------------------

#[sqlx::test]
async fn profile_username_is_unique(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let first = app.sign_up("username-first", "UTC").await;
    let second = app.sign_up("username-second", "UTC").await;
    let username = format!("uniq_{}", &first.id.simple().to_string()[..8]);

    app.json(
        Method::PATCH,
        "/api/profile",
        Some(&first.token),
        json!({ "username": username }),
    )
    .await
    .assert_status(StatusCode::OK);

    // Same username taken by another user → 409.
    app.json(
        Method::PATCH,
        "/api/profile",
        Some(&second.token),
        json!({ "username": username }),
    )
    .await
    .assert_error(StatusCode::CONFLICT, "already taken");

    // `username-check` reflects availability relative to the asking user.
    let taken = app
        .get(
            &format!("/api/profile/username-check?username={username}"),
            Some(&second.token),
        )
        .await;
    taken.assert_status(StatusCode::OK);
    assert_eq!(taken.value_at(&["data", "available"]).as_bool(), Some(false));

    let free_name = format!("free_{}", &second.id.simple().to_string()[..8]);
    let free = app
        .get(
            &format!("/api/profile/username-check?username={free_name}"),
            Some(&second.token),
        )
        .await;
    assert_eq!(free.value_at(&["data", "available"]).as_bool(), Some(true));
}

#[sqlx::test]
async fn onboarding_status_reflects_progress_and_completes_idempotently(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("onboarding", "UTC").await;

    let initial = app.get("/api/onboarding/status", Some(&session.token)).await;
    initial.assert_status(StatusCode::OK);
    assert_eq!(initial.value_at(&["data", "hasSubject"]).as_bool(), Some(false));
    assert_eq!(initial.value_at(&["data", "hasProblem"]).as_bool(), Some(false));

    create_subject(&app, &session).await;
    let after_subject = app.get("/api/onboarding/status", Some(&session.token)).await;
    assert_eq!(after_subject.value_at(&["data", "hasSubject"]).as_bool(), Some(true));
    assert!(
        after_subject
            .value_at(&["data", "firstSubjectId"])
            .as_str()
            .is_some(),
        "firstSubjectId should be set: {}",
        after_subject.body
    );

    // Completing onboarding is idempotent.
    for _ in 0..2 {
        app.json(
            Method::POST,
            "/api/onboarding/complete",
            Some(&session.token),
            json!({}),
        )
        .await
        .assert_status(StatusCode::OK);
    }
}

// ---------------------------------------------------------------------------
// G10 — Admin access control + system health
// ---------------------------------------------------------------------------

async fn set_role(app: &TestApp, user_id: uuid::Uuid, role: &str) {
    sqlx::query("update user_profiles set user_role = $1 where id = $2")
        .bind(role)
        .bind(user_id)
        .execute(&app.pool)
        .await
        .expect("set user role");
}

#[sqlx::test]
async fn admin_endpoints_require_an_admin_role(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let regular = app.sign_up("regular-user", "UTC").await;

    // A plain user is rejected by the `AdminUser` extractor → 403.
    app.get("/api/admin/users", Some(&regular.token))
        .await
        .assert_status(StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn moderator_cannot_access_super_admin_only_endpoints(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let moderator = app.sign_up("moderator", "UTC").await;
    set_role(&app, moderator.id, "moderator").await;

    // `moderator` passes `AdminUser` but `ensure_super` rejects it → 401.
    app.get("/api/admin/users", Some(&moderator.token))
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn super_admin_can_list_users(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let admin = app.sign_up("super-admin", "UTC").await;
    promote_super_admin(&app, admin.id).await;
    // Create a second user so the listing is non-empty.
    app.sign_up("listed-user", "UTC").await;

    let response = app.get("/api/admin/users", Some(&admin.token)).await;
    response.assert_status(StatusCode::OK);
    let users = response.value_at(&["users"]).as_array().expect("users array");
    assert!(
        users.len() >= 2,
        "admin users listing should include multiple users: {}",
        response.body
    );
}

#[sqlx::test]
async fn health_and_readiness_respond(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;

    let health = app.get("/healthz", None).await;
    health.assert_status(StatusCode::OK);
    assert_eq!(health.value_at(&["data", "status"]).as_str(), Some("ok"));

    let ready = app.get("/readyz", None).await;
    ready.assert_status(StatusCode::OK);
    assert_eq!(ready.value_at(&["data", "status"]).as_str(), Some("ready"));
}
