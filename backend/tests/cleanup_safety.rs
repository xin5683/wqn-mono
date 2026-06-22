//! Regression guard for the problem-draft cleanup endpoint.
//!
//! Root cause of the "刷新就没了" bug: the frontend unmount side-effect fired
//! `DELETE /api/problems/{id}/cleanup` right after a successful create, and the
//! old handler deleted the persisted problem row. These tests lock in the fix:
//! cleanup never touches a persisted problem, and it only reclaims orphan draft
//! files for a UUID that has no problem row.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use common::{TestApp, multipart_problem_upload};

#[sqlx::test]
async fn cleanup_leaves_a_persisted_problem_intact(pool: sqlx::PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("owner", "UTC").await;
    let subject_id = common::create_subject(&app, &session).await;
    let problem_id = common::create_problem(&app, &session, subject_id).await;

    // Simulate the buggy unmount firing cleanup immediately after a create.
    let response = app
        .json(
            Method::DELETE,
            &format!("/api/problems/{problem_id}/cleanup"),
            Some(&session.token),
            json!({}),
        )
        .await;
    response.assert_ok();

    // The problem row must survive.
    let count: i64 = sqlx::query_scalar("select count(*) from problems where id = $1")
        .bind(problem_id)
        .fetch_one(&app.pool)
        .await
        .expect("problem count");
    assert_eq!(count, 1, "cleanup deleted the persisted problem row");

    let response = app
        .get(&format!("/api/problems/{problem_id}"), Some(&session.token))
        .await;
    response.assert_ok();
}

#[sqlx::test]
async fn cleanup_reclaims_orphan_draft_files(pool: sqlx::PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("owner", "UTC").await;

    // A draft UUID that was never persisted as a problem, with an uploaded file
    // sitting on disk under its draft directory.
    let draft_id = Uuid::new_v4();
    let boundary = "----wqn-test-boundary";
    let body = multipart_problem_upload(boundary, draft_id, "image.png", "image/png", "png-bytes");
    let response = app
        .multipart("/api/files/upload", Some(&session.token), boundary, body)
        .await;
    response.assert_ok();
    let path = response
        .data()
        .get("paths")
        .and_then(|paths| paths.get(0))
        .and_then(|value| value.as_str())
        .expect("upload returns paths[0]")
        .to_owned();

    let response = app
        .json(
            Method::DELETE,
            &format!("/api/problems/{draft_id}/cleanup"),
            Some(&session.token),
            json!({}),
        )
        .await;
    response.assert_ok();

    // The orphan draft file is gone (the file endpoint returns a JSON 404 once
    // the object is missing), and no problem row was created.
    let response = app
        .get(&format!("/api/files/{path}"), Some(&session.token))
        .await;
    assert_eq!(
        response.status,
        StatusCode::NOT_FOUND,
        "orphan draft file was not reclaimed"
    );

    let count: i64 = sqlx::query_scalar("select count(*) from problems where id = $1")
        .bind(draft_id)
        .fetch_one(&app.pool)
        .await
        .expect("problem count");
    assert_eq!(count, 0, "cleanup created a problem row");
}
