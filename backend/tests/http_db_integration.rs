use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

mod common;
use common::*;

#[sqlx::test]
async fn problem_dto_responses_preserve_tags_and_assets(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("problem-dto", "UTC").await;
    let subject_id = create_subject(&app, &session).await;
    let tag_id = create_tag(&app, &session, subject_id, "dto-tag").await;
    let problem_id = Uuid::new_v4();
    let asset_path = format!(
        "user/{}/problems/{problem_id}/problem/question.png",
        session.id
    );
    let solution_asset_path = format!(
        "user/{}/problems/{problem_id}/solution/explanation.png",
        session.id
    );

    let created = app
        .json(
            Method::POST,
            "/api/problems",
            Some(&session.token),
            json!({
                "id": problem_id,
                "subject_id": subject_id,
                "title": "Typed DTO problem",
                "content": "Question body",
                "problem_type": "mcq",
                "correct_answer": "A",
                "answer_config": {
                    "type": "mcq",
                    "choices": [{ "id": "a", "text": "A" }, { "id": "b", "text": "B" }],
                    "correct_choice_id": "a",
                    "randomize_choices": false
                },
                "auto_mark": true,
                "status": "wrong",
                "assets": [{ "path": asset_path, "kind": "image" }],
                "solution_text": "Because A",
                "solution_assets": [{ "path": solution_asset_path, "kind": "image" }],
                "tag_ids": [tag_id],
            }),
        )
        .await;
    created.assert_status(StatusCode::CREATED);
    assert_problem_dto_shape(&created, problem_id, subject_id, session.id, tag_id);

    let detail = app
        .get(&format!("/api/problems/{problem_id}"), Some(&session.token))
        .await;
    detail.assert_status(StatusCode::OK);
    assert_problem_dto_shape(&detail, problem_id, subject_id, session.id, tag_id);

    let list = app
        .get(
            &format!("/api/problems?subject_id={subject_id}"),
            Some(&session.token),
        )
        .await;
    list.assert_status(StatusCode::OK);
    let problems = list.value_at(&["data"]).as_array().expect("problem list");
    assert_eq!(problems.len(), 1, "list response: {}", list.body);
    let listed = TestResponse {
        status: StatusCode::OK,
        body: json!({ "data": problems[0] }),
    };
    assert_problem_dto_shape(&listed, problem_id, subject_id, session.id, tag_id);
}

fn assert_problem_dto_shape(
    response: &TestResponse,
    problem_id: Uuid,
    subject_id: Uuid,
    user_id: Uuid,
    tag_id: Uuid,
) {
    let problem_id = problem_id.to_string();
    let subject_id = subject_id.to_string();
    let user_id = user_id.to_string();
    let tag_id = tag_id.to_string();

    assert_eq!(
        response.value_at(&["data", "id"]).as_str(),
        Some(problem_id.as_str())
    );
    assert_eq!(
        response.value_at(&["data", "subject_id"]).as_str(),
        Some(subject_id.as_str())
    );
    assert_eq!(
        response.value_at(&["data", "user_id"]).as_str(),
        Some(user_id.as_str())
    );
    assert_eq!(
        response.value_at(&["data", "title"]).as_str(),
        Some("Typed DTO problem")
    );
    let data = response.value_at(&["data"]);
    let choices = data
        .get("answer_config")
        .and_then(|value| value.get("choices"))
        .and_then(Value::as_array)
        .expect("answer choices");
    assert_eq!(
        choices
            .first()
            .and_then(|choice| choice.get("text"))
            .and_then(Value::as_str),
        Some("A")
    );
    let assets = data
        .get("assets")
        .and_then(Value::as_array)
        .expect("assets");
    assert_eq!(
        assets
            .first()
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str),
        Some("image")
    );
    let solution_assets = data
        .get("solution_assets")
        .and_then(Value::as_array)
        .expect("solution assets");
    assert_eq!(
        solution_assets
            .first()
            .and_then(|value| value.get("kind"))
            .and_then(Value::as_str),
        Some("image")
    );
    let tags = data.get("tags").and_then(Value::as_array).expect("tags");
    let tag = tags.first().expect("first tag");
    assert_eq!(tag.get("id").and_then(Value::as_str), Some(tag_id.as_str()));
    assert_eq!(tag.get("name").and_then(Value::as_str), Some("dto-tag"));
    assert_eq!(
        tag.get("user_id").and_then(Value::as_str),
        Some(user_id.as_str())
    );
}

#[sqlx::test]
async fn problem_patch_distinguishes_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("problem-patch", "UTC").await;
    let subject_id = create_subject(&app, &session).await;
    let tag_id = create_tag(&app, &session, subject_id, "clear-me").await;
    let problem_id = Uuid::new_v4();
    let asset_path = format!(
        "user/{}/problems/{problem_id}/problem/question.png",
        session.id
    );
    let solution_asset_path = format!(
        "user/{}/problems/{problem_id}/solution/explanation.png",
        session.id
    );

    let created = app
        .json(
            Method::POST,
            "/api/problems",
            Some(&session.token),
            json!({
                "id": problem_id,
                "subject_id": subject_id,
                "title": "Patch semantics",
                "content": "Clear this content",
                "problem_type": "short",
                "correct_answer": "clear me",
                "answer_config": {
                    "type": "short",
                    "mode": "text",
                    "acceptable_answers": ["clear me"]
                },
                "status": "wrong",
                "assets": [{ "path": asset_path }],
                "solution_text": "Clear this solution",
                "solution_assets": [{ "path": solution_asset_path }],
                "last_reviewed_date": "2026-01-01T00:00:00Z",
                "tag_ids": [tag_id],
            }),
        )
        .await;
    created.assert_status(StatusCode::CREATED);

    let cleared = app
        .json(
            Method::PATCH,
            &format!("/api/problems/{problem_id}"),
            Some(&session.token),
            json!({
                "content": null,
                "correct_answer": null,
                "answer_config": null,
                "assets": null,
                "solution_text": null,
                "solution_assets": null,
                "last_reviewed_date": null,
                "tag_ids": null,
            }),
        )
        .await;
    cleared.assert_status(StatusCode::OK);
    let data = cleared.value_at(&["data"]);
    assert_eq!(
        data.get("title").and_then(Value::as_str),
        Some("Patch semantics")
    );
    assert_eq!(data.get("status").and_then(Value::as_str), Some("wrong"));
    assert!(data.get("content").is_some_and(Value::is_null));
    assert!(data.get("correct_answer").is_some_and(Value::is_null));
    assert!(data.get("answer_config").is_some_and(Value::is_null));
    assert!(data.get("solution_text").is_some_and(Value::is_null));
    assert!(data.get("last_reviewed_date").is_some_and(Value::is_null));
    assert_empty_array_field(data, "assets");
    assert_empty_array_field(data, "solution_assets");
    assert_empty_array_field(data, "tags");

    let updated = app
        .json(
            Method::PATCH,
            &format!("/api/problems/{problem_id}"),
            Some(&session.token),
            json!({ "content": "New content only" }),
        )
        .await;
    updated.assert_status(StatusCode::OK);
    let data = updated.value_at(&["data"]);
    assert_eq!(
        data.get("content").and_then(Value::as_str),
        Some("New content only")
    );
    assert_eq!(
        data.get("title").and_then(Value::as_str),
        Some("Patch semantics")
    );
    assert!(data.get("correct_answer").is_some_and(Value::is_null));

    app.json(
        Method::PATCH,
        &format!("/api/problems/{problem_id}"),
        Some(&session.token),
        json!({ "title": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let assets_restored = app
        .json(
            Method::PATCH,
            &format!("/api/problems/{problem_id}/assets"),
            Some(&session.token),
            json!({
                "assets": [{ "path": asset_path }],
                "solution_assets": [{ "path": solution_asset_path }],
            }),
        )
        .await;
    assets_restored.assert_status(StatusCode::OK);
    assert_eq!(
        assets_restored
            .value_at(&["data"])
            .get("assets")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let assets_cleared = app
        .json(
            Method::PATCH,
            &format!("/api/problems/{problem_id}/assets"),
            Some(&session.token),
            json!({ "assets": null, "solution_assets": null }),
        )
        .await;
    assets_cleared.assert_status(StatusCode::OK);
    let data = assets_cleared.value_at(&["data"]);
    assert_empty_array_field(data, "assets");
    assert_empty_array_field(data, "solution_assets");
}

fn assert_empty_array_field(data: &Value, key: &str) {
    assert!(
        data.get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| items.is_empty()),
        "expected {key} to be an empty array in {data}"
    );
}

#[sqlx::test]
async fn profile_patch_distinguishes_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("profile-patch", "UTC").await;

    let updated = app
        .json(
            Method::PATCH,
            "/api/profile",
            Some(&session.token),
            json!({
                "username": format!("profile_{}", session.id.simple()),
                "first_name": "Ada",
                "last_name": "Lovelace",
                "date_of_birth": "1815-12-10",
                "gender": "female",
                "region": "London",
                "timezone": "Asia/Shanghai",
                "avatar_url": "/api/files/avatars/avatar.png",
                "bio": "Initial bio",
            }),
        )
        .await;
    updated.assert_status(StatusCode::OK);
    assert_eq!(
        updated.value_at(&["data", "timezone"]).as_str(),
        Some("Asia/Shanghai")
    );
    assert_eq!(
        updated.value_at(&["data", "date_of_birth"]).as_str(),
        Some("1815-12-10")
    );

    let cleared = app
        .json(
            Method::PATCH,
            "/api/profile",
            Some(&session.token),
            json!({
                "username": null,
                "first_name": null,
                "date_of_birth": null,
                "gender": null,
                "avatar_url": null,
                "bio": null,
            }),
        )
        .await;
    cleared.assert_status(StatusCode::OK);
    let data = cleared.value_at(&["data"]);
    assert!(data.get("username").is_some_and(Value::is_null));
    assert!(data.get("first_name").is_some_and(Value::is_null));
    assert!(data.get("date_of_birth").is_some_and(Value::is_null));
    assert!(data.get("gender").is_some_and(Value::is_null));
    assert!(data.get("avatar_url").is_some_and(Value::is_null));
    assert!(data.get("bio").is_some_and(Value::is_null));
    assert_eq!(
        data.get("last_name").and_then(Value::as_str),
        Some("Lovelace")
    );
    assert_eq!(
        data.get("timezone").and_then(Value::as_str),
        Some("Asia/Shanghai")
    );

    let kept = app
        .json(
            Method::PATCH,
            "/api/profile",
            Some(&session.token),
            json!({ "region": "" }),
        )
        .await;
    kept.assert_status(StatusCode::OK);
    let data = kept.value_at(&["data"]);
    assert!(data.get("region").is_some_and(Value::is_null));
    assert_eq!(
        data.get("last_name").and_then(Value::as_str),
        Some("Lovelace")
    );

    app.json(
        Method::PATCH,
        "/api/profile",
        Some(&session.token),
        json!({ "timezone": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        "/api/profile",
        Some(&session.token),
        json!({ "timezone": "" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        "/api/profile",
        Some(&session.token),
        json!({ "date_of_birth": "not-a-date" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn attempt_patch_distinguishes_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("attempt-patch", "UTC").await;
    let subject_id = create_subject(&app, &session).await;
    let problem_id = create_problem(&app, &session, subject_id).await;

    let created = app
        .json(
            Method::POST,
            "/api/attempts",
            Some(&session.token),
            json!({
                "problem_id": problem_id,
                "submitted_answer": { "text": "original" },
                "is_self_assessed": true,
                "confidence": 3,
                "cause": "careless",
                "reflection_notes": "initial notes",
                "selected_status": "wrong",
            }),
        )
        .await;
    created.assert_status(StatusCode::CREATED);
    let attempt_id = created.uuid_at(&["data", "id"]);

    let cleared = app
        .json(
            Method::PATCH,
            &format!("/api/attempts/{attempt_id}"),
            Some(&session.token),
            json!({
                "confidence": null,
                "cause": null,
                "reflection_notes": null,
                "selected_status": null,
            }),
        )
        .await;
    cleared.assert_status(StatusCode::OK);
    let data = cleared.value_at(&["data"]);
    assert_eq!(
        data.get("submitted_answer")
            .and_then(|answer| answer.get("text"))
            .and_then(Value::as_str),
        Some("original")
    );
    assert!(data.get("confidence").is_some_and(Value::is_null));
    assert!(data.get("cause").is_some_and(Value::is_null));
    assert!(data.get("reflection_notes").is_some_and(Value::is_null));
    assert!(data.get("selected_status").is_some_and(Value::is_null));

    let updated = app
        .json(
            Method::PATCH,
            &format!("/api/attempts/{attempt_id}"),
            Some(&session.token),
            json!({
                "submitted_answer": { "text": "updated" },
                "confidence": 5,
                "selected_status": "mastered",
            }),
        )
        .await;
    updated.assert_status(StatusCode::OK);
    let data = updated.value_at(&["data"]);
    assert_eq!(
        data.get("submitted_answer")
            .and_then(|answer| answer.get("text"))
            .and_then(Value::as_str),
        Some("updated")
    );
    assert_eq!(data.get("confidence").and_then(Value::as_i64), Some(5));
    assert_eq!(
        data.get("selected_status").and_then(Value::as_str),
        Some("mastered")
    );
    assert!(data.get("cause").is_some_and(Value::is_null));

    app.json(
        Method::PATCH,
        &format!("/api/attempts/{attempt_id}"),
        Some(&session.token),
        json!({ "submitted_answer": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/attempts/{attempt_id}"),
        Some(&session.token),
        json!({ "confidence": 6 }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/attempts/{attempt_id}"),
        Some(&session.token),
        json!({ "selected_status": "invalid" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn auto_mark_attempt_is_graded_server_side(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("auto-grade", "UTC").await;
    let subject_id = create_subject(&app, &session).await;

    // Auto-mark MCQ whose correct choice is "a". The submit path does NOT send
    // is_correct — the backend must grade it.
    let problem = app
        .json(
            Method::POST,
            "/api/problems",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "title": "MCQ grade",
                "problem_type": "mcq",
                "status": "needs_review",
                "auto_mark": true,
                "answer_config": {
                    "type": "mcq",
                    "choices": [
                        { "id": "a", "text": "Alpha" },
                        { "id": "b", "text": "Beta" }
                    ],
                    "correct_choice_id": "a",
                    "randomize_choices": false
                }
            }),
        )
        .await;
    problem.assert_status(StatusCode::CREATED);
    let problem_id = problem.uuid_at(&["data", "id"]);

    // Correct submission.
    let correct = app
        .json(
            Method::POST,
            &format!("/api/problems/{problem_id}/attempt"),
            Some(&session.token),
            json!({ "submitted_answer": "a" }),
        )
        .await;
    correct.assert_status(StatusCode::CREATED);
    let data = correct.value_at(&["data"]);
    assert_eq!(data.get("is_correct").and_then(Value::as_bool), Some(true));
    // The auto-mark submit leaves the attempt row's selected_status NULL;
    // the user confirms the status via the assessment form. The derived
    // status is applied to the problem instead.
    let attempt_status = data.get("selected_status");
    assert!(attempt_status.is_none() || attempt_status.is_some_and(Value::is_null));

    let fetched = app
        .get(&format!("/api/problems/{problem_id}"), Some(&session.token))
        .await;
    fetched.assert_status(StatusCode::OK);
    assert_eq!(
        fetched.value_at(&["data"]).get("status").and_then(Value::as_str),
        Some("mastered")
    );

    // Incorrect submission.
    let wrong = app
        .json(
            Method::POST,
            &format!("/api/problems/{problem_id}/attempt"),
            Some(&session.token),
            json!({ "submitted_answer": "b" }),
        )
        .await;
    wrong.assert_status(StatusCode::CREATED);
    let data = wrong.value_at(&["data"]);
    assert_eq!(data.get("is_correct").and_then(Value::as_bool), Some(false));

    let fetched = app
        .get(&format!("/api/problems/{problem_id}"), Some(&session.token))
        .await;
    fetched.assert_status(StatusCode::OK);
    assert_eq!(
        fetched.value_at(&["data"]).get("status").and_then(Value::as_str),
        Some("wrong")
    );

    // Non-auto-mark problem: no client is_correct and no grading → NULL.
    let manual = app
        .json(
            Method::POST,
            "/api/problems",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "title": "Manual",
                "problem_type": "extended",
                "status": "needs_review",
                "auto_mark": false
            }),
        )
        .await;
    manual.assert_status(StatusCode::CREATED);
    let manual_id = manual.uuid_at(&["data", "id"]);
    let attempt = app
        .json(
            Method::POST,
            &format!("/api/problems/{manual_id}/attempt"),
            Some(&session.token),
            json!({ "submitted_answer": "anything" }),
        )
        .await;
    attempt.assert_status(StatusCode::CREATED);
    let data = attempt.value_at(&["data"]);
    assert!(data.get("is_correct").is_some_and(Value::is_null));
    let status = data.get("selected_status");
    assert!(status.is_none() || status.is_some_and(Value::is_null));

    // Client-supplied is_correct is always trusted (self-assessment path),
    // even on auto-mark problems.
    let trusted = app
        .json(
            Method::POST,
            &format!("/api/problems/{problem_id}/attempt"),
            Some(&session.token),
            json!({ "submitted_answer": "b", "is_correct": true, "is_self_assessed": true }),
        )
        .await;
    trusted.assert_status(StatusCode::CREATED);
    assert_eq!(
        trusted.value_at(&["data"]).get("is_correct").and_then(Value::as_bool),
        Some(true)
    );
}

#[sqlx::test]
async fn auto_mark_short_answer_numeric_is_graded(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("auto-grade-num", "UTC").await;
    let subject_id = create_subject(&app, &session).await;

    let problem = app
        .json(
            Method::POST,
            "/api/problems",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "title": "Numeric grade",
                "problem_type": "short",
                "status": "needs_review",
                "auto_mark": true,
                "answer_config": {
                    "type": "short",
                    "mode": "numeric",
                    "numeric_config": { "correct_value": 3.14, "tolerance": 0.01 }
                }
            }),
        )
        .await;
    problem.assert_status(StatusCode::CREATED);
    let problem_id = problem.uuid_at(&["data", "id"]);

    let within = app
        .json(
            Method::POST,
            &format!("/api/problems/{problem_id}/attempt"),
            Some(&session.token),
            json!({ "submitted_answer": "3.141" }),
        )
        .await;
    within.assert_status(StatusCode::CREATED);
    assert_eq!(
        within
            .value_at(&["data"])
            .get("is_correct")
            .and_then(Value::as_bool),
        Some(true)
    );

    let outside = app
        .json(
            Method::POST,
            &format!("/api/problems/{problem_id}/attempt"),
            Some(&session.token),
            json!({ "submitted_answer": "5" }),
        )
        .await;
    outside.assert_status(StatusCode::CREATED);
    assert_eq!(
        outside
            .value_at(&["data"])
            .get("is_correct")
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[sqlx::test]
async fn subject_and_tag_patch_distinguish_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("subject-tag-patch", "UTC").await;

    let subject = app
        .json(
            Method::POST,
            "/api/subjects",
            Some(&session.token),
            json!({
                "name": "Patch Subject",
                "color": "#112233",
                "icon": "BookOpen",
            }),
        )
        .await;
    subject.assert_status(StatusCode::CREATED);
    let subject_id = subject.uuid_at(&["data", "id"]);

    let patched_subject = app
        .json(
            Method::PATCH,
            &format!("/api/subjects/{subject_id}"),
            Some(&session.token),
            json!({ "color": null }),
        )
        .await;
    patched_subject.assert_status(StatusCode::OK);
    assert_eq!(
        patched_subject.value_at(&["data", "name"]).as_str(),
        Some("Patch Subject")
    );
    assert!(patched_subject.value_at(&["data", "color"]).is_null());
    assert_eq!(
        patched_subject.value_at(&["data", "icon"]).as_str(),
        Some("BookOpen")
    );

    app.json(
        Method::PATCH,
        &format!("/api/subjects/{subject_id}"),
        Some(&session.token),
        json!({ "name": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/subjects/{subject_id}"),
        Some(&session.token),
        json!({ "name": "" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let tag_id = create_tag(&app, &session, subject_id, "Patch Tag").await;
    let unchanged_tag = app
        .json(
            Method::PATCH,
            &format!("/api/tags/{tag_id}"),
            Some(&session.token),
            json!({}),
        )
        .await;
    unchanged_tag.assert_status(StatusCode::OK);
    assert_eq!(
        unchanged_tag.value_at(&["data", "name"]).as_str(),
        Some("Patch Tag")
    );

    let renamed_tag = app
        .json(
            Method::PATCH,
            &format!("/api/tags/{tag_id}"),
            Some(&session.token),
            json!({ "name": "Renamed Tag" }),
        )
        .await;
    renamed_tag.assert_status(StatusCode::OK);
    assert_eq!(
        renamed_tag.value_at(&["data", "name"]).as_str(),
        Some("Renamed Tag")
    );

    app.json(
        Method::PATCH,
        &format!("/api/tags/{tag_id}"),
        Some(&session.token),
        json!({ "name": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/tags/{tag_id}"),
        Some(&session.token),
        json!({ "name": "" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn problem_set_dto_responses_preserve_details_favourites_and_links(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("problem-set-dto", "UTC").await;
    let subject_id = create_subject(&app, &session).await;
    let tag_id = create_tag(&app, &session, subject_id, "set-dto-tag").await;
    let problem_id = create_problem_with(
        &app,
        &session,
        subject_id,
        "short",
        "needs_review",
        vec![tag_id],
    )
    .await;

    let created = app
        .json(
            Method::POST,
            "/api/problem-sets",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "name": "Typed DTO problem set",
                "description": "Typed DTO description",
                "sharing_level": "limited",
                "shared_with_emails": ["learner@example.test"],
                "problem_ids": [problem_id],
                "session_config": { "randomize": false },
                "allow_copying": true,
            }),
        )
        .await;
    created.assert_status(StatusCode::CREATED);
    let problem_set_id = created.uuid_at(&["data", "id"]);
    assert_problem_set_dto_shape(
        &created,
        problem_set_id,
        subject_id,
        session.id,
        problem_id,
        tag_id,
    );

    let detail = app
        .get(
            &format!("/api/problem-sets/{problem_set_id}"),
            Some(&session.token),
        )
        .await;
    detail.assert_status(StatusCode::OK);
    assert_problem_set_dto_shape(
        &detail,
        problem_set_id,
        subject_id,
        session.id,
        problem_id,
        tag_id,
    );

    let list = app.get("/api/problem-sets", Some(&session.token)).await;
    list.assert_status(StatusCode::OK);
    let listed_sets = list
        .value_at(&["data"])
        .as_array()
        .expect("problem set list");
    let listed = find_value_by_id(listed_sets, problem_set_id);
    assert_eq!(listed.get("problem_count").and_then(Value::as_i64), Some(1));
    assert!(listed.get("subject_name").and_then(Value::as_str).is_some());
    assert!(
        listed.get("problems").is_none(),
        "list response should stay compact: {}",
        list.body
    );

    let set_problems = app
        .get(
            &format!("/api/problem-sets/{problem_set_id}/problems"),
            Some(&session.token),
        )
        .await;
    set_problems.assert_status(StatusCode::OK);
    let linked = set_problems
        .value_at(&["data"])
        .as_array()
        .and_then(|items| items.first())
        .expect("linked problem");
    let problem_id_string = problem_id.to_string();
    let tag_id_string = tag_id.to_string();
    assert_eq!(
        linked.get("problem_id").and_then(Value::as_str),
        Some(problem_id_string.as_str())
    );
    assert!(linked.get("added_at").and_then(Value::as_str).is_some());
    assert_eq!(
        linked
            .get("problems")
            .and_then(|problem| problem.get("tags"))
            .and_then(Value::as_array)
            .and_then(|tags| tags.first())
            .and_then(|tag| tag.get("id"))
            .and_then(Value::as_str),
        Some(tag_id_string.as_str())
    );

    app.json(
        Method::POST,
        &format!("/api/problem-sets/{problem_set_id}/favourite"),
        Some(&session.token),
        json!({}),
    )
    .await
    .assert_status(StatusCode::OK);
    let favourites = app
        .get("/api/problem-sets/favourites", Some(&session.token))
        .await;
    favourites.assert_status(StatusCode::OK);
    let favourite_sets = favourites
        .value_at(&["data"])
        .as_array()
        .expect("favourites");
    let favourite = find_value_by_id(favourite_sets, problem_set_id);
    assert!(
        favourite
            .get("favorited_at")
            .and_then(Value::as_str)
            .is_some()
    );
    assert!(
        favourite
            .get("favourited_at")
            .and_then(Value::as_str)
            .is_some()
    );
}

fn assert_problem_set_dto_shape(
    response: &TestResponse,
    problem_set_id: Uuid,
    subject_id: Uuid,
    user_id: Uuid,
    problem_id: Uuid,
    tag_id: Uuid,
) {
    let data = response.value_at(&["data"]);
    let problem_set_id = problem_set_id.to_string();
    let subject_id = subject_id.to_string();
    let user_id = user_id.to_string();
    let problem_id = problem_id.to_string();
    let tag_id = tag_id.to_string();

    assert_eq!(
        data.get("id").and_then(Value::as_str),
        Some(problem_set_id.as_str())
    );
    assert_eq!(
        data.get("subject_id").and_then(Value::as_str),
        Some(subject_id.as_str())
    );
    assert_eq!(
        data.get("user_id").and_then(Value::as_str),
        Some(user_id.as_str())
    );
    assert_eq!(
        data.get("name").and_then(Value::as_str),
        Some("Typed DTO problem set")
    );
    assert_eq!(data.get("problem_count").and_then(Value::as_i64), Some(1));
    assert!(data.get("subject_name").and_then(Value::as_str).is_some());
    assert_eq!(
        data.get("shared_with_emails")
            .and_then(Value::as_array)
            .and_then(|emails| emails.first())
            .and_then(Value::as_str),
        Some("learner@example.test")
    );
    assert_eq!(
        data.get("owner_profile")
            .and_then(|profile| profile.get("id"))
            .and_then(Value::as_str),
        Some(user_id.as_str())
    );
    assert_eq!(
        data.get("problems")
            .and_then(Value::as_array)
            .and_then(|problems| problems.first())
            .and_then(|link| link.get("problem_id"))
            .and_then(Value::as_str),
        Some(problem_id.as_str())
    );
    assert_eq!(
        data.get("problems")
            .and_then(Value::as_array)
            .and_then(|problems| problems.first())
            .and_then(|link| link.get("problems"))
            .and_then(|problem| problem.get("id"))
            .and_then(Value::as_str),
        Some(problem_id.as_str())
    );
    assert_eq!(
        data.get("problems")
            .and_then(Value::as_array)
            .and_then(|problems| problems.first())
            .and_then(|link| link.get("problems"))
            .and_then(|problem| problem.get("tags"))
            .and_then(Value::as_array)
            .and_then(|tags| tags.first())
            .and_then(|tag| tag.get("id"))
            .and_then(Value::as_str),
        Some(tag_id.as_str())
    );
}

fn find_value_by_id(values: &[Value], id: Uuid) -> &Value {
    let id = id.to_string();
    values
        .iter()
        .find(|value| value.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .unwrap_or_else(|| panic!("missing value with id {id}: {values:?}"))
}

#[sqlx::test]
async fn problem_set_put_distinguishes_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("problem-set-patch", "UTC").await;
    let subject_id = create_subject(&app, &session).await;

    let created = app
        .json(
            Method::POST,
            "/api/problem-sets",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "name": "Patch Problem Set",
                "description": "Clear this description",
                "sharing_level": "limited",
                "shared_with_emails": ["friend@example.test"],
                "session_config": { "randomize": true },
                "allow_copying": true,
            }),
        )
        .await;
    created.assert_status(StatusCode::CREATED);
    let problem_set_id = created.uuid_at(&["data", "id"]);

    let cleared = app
        .json(
            Method::PUT,
            &format!("/api/problem-sets/{problem_set_id}"),
            Some(&session.token),
            json!({
                "description": null,
                "session_config": null,
                "sharing_level": "limited",
                "shared_with_emails": null,
                "allow_copying": false,
            }),
        )
        .await;
    cleared.assert_status(StatusCode::OK);
    let data = cleared.value_at(&["data"]);
    assert_eq!(
        data.get("name").and_then(Value::as_str),
        Some("Patch Problem Set")
    );
    assert!(data.get("description").is_some_and(Value::is_null));
    assert!(data.get("session_config").is_some_and(Value::is_null));
    assert_eq!(
        data.get("allow_copying").and_then(Value::as_bool),
        Some(false)
    );
    assert_empty_array_field(data, "shared_with_emails");

    let updated = app
        .json(
            Method::PUT,
            &format!("/api/problem-sets/{problem_set_id}"),
            Some(&session.token),
            json!({
                "name": "Renamed Problem Set",
                "description": "New description",
                "sharing_level": "public",
                "is_listed": true,
                "discovery_subject": "Mathematics",
            }),
        )
        .await;
    updated.assert_status(StatusCode::OK);
    let data = updated.value_at(&["data"]);
    assert_eq!(
        data.get("name").and_then(Value::as_str),
        Some("Renamed Problem Set")
    );
    assert_eq!(
        data.get("description").and_then(Value::as_str),
        Some("New description")
    );
    assert_eq!(
        data.get("discovery_subject").and_then(Value::as_str),
        Some("Mathematics")
    );
    assert_eq!(data.get("is_listed").and_then(Value::as_bool), Some(true));

    let hidden = app
        .json(
            Method::PUT,
            &format!("/api/problem-sets/{problem_set_id}"),
            Some(&session.token),
            json!({ "discovery_subject": null }),
        )
        .await;
    hidden.assert_status(StatusCode::OK);
    assert!(
        hidden
            .value_at(&["data"])
            .get("discovery_subject")
            .is_some_and(Value::is_null)
    );

    app.json(
        Method::PUT,
        &format!("/api/problem-sets/{problem_set_id}"),
        Some(&session.token),
        json!({ "name": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PUT,
        &format!("/api/problem-sets/{problem_set_id}"),
        Some(&session.token),
        json!({ "sharing_level": "invalid" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PUT,
        &format!("/api/problem-sets/{problem_set_id}"),
        Some(&session.token),
        json!({ "is_smart": true }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn problem_filter_sql_is_shared_by_lists_counts_and_smart_sets(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("problem-filter", "UTC").await;
    let subject_id = create_subject(&app, &session).await;
    let keep_tag_id = create_tag(&app, &session, subject_id, "keep").await;
    let other_tag_id = create_tag(&app, &session, subject_id, "other").await;

    let matching_problem = create_problem_with(
        &app,
        &session,
        subject_id,
        "short",
        "wrong",
        vec![keep_tag_id],
    )
    .await;
    let wrong_status = create_problem_with(
        &app,
        &session,
        subject_id,
        "short",
        "mastered",
        vec![keep_tag_id],
    )
    .await;
    let wrong_tag = create_problem_with(
        &app,
        &session,
        subject_id,
        "short",
        "wrong",
        vec![other_tag_id],
    )
    .await;

    let list = app
        .get(
            &format!(
                "/api/problems?subject_id={subject_id}&statuses=wrong&problem_types=short&tag_ids={keep_tag_id}&tag_filter_mode=all"
            ),
            Some(&session.token),
        )
        .await;
    list.assert_status(StatusCode::OK);
    let listed = list.value_at(&["data"]).as_array().expect("problem list");
    assert_eq!(listed.len(), 1, "list response: {}", list.body);
    let matching_problem_id = matching_problem.to_string();
    assert_eq!(
        listed[0].get("id").and_then(Value::as_str),
        Some(matching_problem_id.as_str())
    );

    let count = app
        .json(
            Method::POST,
            "/api/problems/filter-count",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "filter_config": {
                    "statuses": ["wrong"],
                    "problem_types": ["short"],
                    "tag_ids": [keep_tag_id],
                    "tag_filter_mode": "all"
                }
            }),
        )
        .await;
    count.assert_status(StatusCode::OK);
    assert_eq!(count.value_at(&["data", "count"]).as_i64(), Some(1));

    let smart_set = app
        .json(
            Method::POST,
            "/api/problem-sets",
            Some(&session.token),
            json!({
                "subject_id": subject_id,
                "name": "Smart filter parity",
                "is_smart": true,
                "filter_config": {
                    "statuses": ["wrong"],
                    "problem_types": ["short"],
                    "tag_ids": [keep_tag_id],
                    "tag_filter_mode": "all"
                }
            }),
        )
        .await;
    smart_set.assert_status(StatusCode::CREATED);
    let smart_set_id = smart_set.uuid_at(&["data", "id"]);
    let smart_set = app
        .get(
            &format!("/api/problem-sets/{smart_set_id}"),
            Some(&session.token),
        )
        .await;
    smart_set.assert_status(StatusCode::OK);
    let smart_problems = smart_set
        .value_at(&["data", "problems"])
        .as_array()
        .expect("smart set problems");
    assert_eq!(
        smart_problems.len(),
        1,
        "smart set response: {}",
        smart_set.body
    );
    assert_eq!(
        smart_problems[0].get("problem_id").and_then(Value::as_str),
        Some(matching_problem_id.as_str())
    );
    assert!(
        !smart_set
            .body
            .to_string()
            .contains(&wrong_status.to_string())
            && !smart_set.body.to_string().contains(&wrong_tag.to_string()),
        "smart set should exclude non-matching problems: {}",
        smart_set.body
    );
}

#[sqlx::test]
async fn admin_disable_restore_invalidates_existing_user_tokens(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let admin = app.sign_up("admin-disable-admin", "UTC").await;
    let target = app.sign_up("admin-disable-target", "UTC").await;
    promote_super_admin(&app, admin.id).await;

    let disabled = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}", target.id),
            Some(&admin.token),
            json!({ "is_active": false }),
        )
        .await;
    disabled.assert_status(StatusCode::OK);
    assert_eq!(
        disabled.value_at(&["data", "is_active"]).as_bool(),
        Some(false)
    );
    assert!(
        !disabled.value_at(&["data", "disabled_at"]).is_null(),
        "disabled_at should be set: {}",
        disabled.body
    );
    app.get("/api/auth/me", Some(&target.token))
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    app.json(
        Method::POST,
        "/api/auth/login",
        None,
        json!({ "email": target.email, "password": "password123" }),
    )
    .await
    .assert_status(StatusCode::UNAUTHORIZED);

    let restored = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}", target.id),
            Some(&admin.token),
            json!({ "is_active": true }),
        )
        .await;
    restored.assert_status(StatusCode::OK);
    assert_eq!(
        restored.value_at(&["data", "is_active"]).as_bool(),
        Some(true)
    );
    assert!(restored.value_at(&["data", "disabled_at"]).is_null());
    app.get("/api/auth/me", Some(&target.token))
        .await
        .assert_status(StatusCode::UNAUTHORIZED);
    app.json(
        Method::POST,
        "/api/auth/login",
        None,
        json!({ "email": target.email, "password": "password123" }),
    )
    .await
    .assert_status(StatusCode::OK);
}

#[sqlx::test]
async fn admin_user_patch_distinguishes_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let admin = app.sign_up("admin-user-patch-admin", "UTC").await;
    let target = app.sign_up("admin-user-patch-target", "UTC").await;
    promote_super_admin(&app, admin.id).await;

    let updated = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}", target.id),
            Some(&admin.token),
            json!({
                "username": "patch_target",
                "user_role": "moderator",
                "is_active": false,
            }),
        )
        .await;
    updated.assert_status(StatusCode::OK);
    let data = updated.value_at(&["data"]);
    assert_eq!(
        data.get("username").and_then(Value::as_str),
        Some("patch_target")
    );
    assert_eq!(
        data.get("user_role").and_then(Value::as_str),
        Some("moderator")
    );
    assert_eq!(data.get("is_active").and_then(Value::as_bool), Some(false));

    let cleared = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}", target.id),
            Some(&admin.token),
            json!({ "username": null }),
        )
        .await;
    cleared.assert_status(StatusCode::OK);
    let data = cleared.value_at(&["data"]);
    assert!(data.get("username").is_some_and(Value::is_null));
    assert_eq!(
        data.get("user_role").and_then(Value::as_str),
        Some("moderator")
    );
    assert_eq!(data.get("is_active").and_then(Value::as_bool), Some(false));

    let restored = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}", target.id),
            Some(&admin.token),
            json!({ "username": "", "is_active": true }),
        )
        .await;
    restored.assert_status(StatusCode::OK);
    let data = restored.value_at(&["data"]);
    assert!(data.get("username").is_some_and(Value::is_null));
    assert_eq!(
        data.get("user_role").and_then(Value::as_str),
        Some("moderator")
    );
    assert_eq!(data.get("is_active").and_then(Value::as_bool), Some(true));

    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}", target.id),
        Some(&admin.token),
        json!({ "user_role": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}", target.id),
        Some(&admin.token),
        json!({ "user_role": "owner" }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}", target.id),
        Some(&admin.token),
        json!({ "is_active": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let role_alias = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/role", target.id),
            Some(&admin.token),
            json!({ "user_role": "admin" }),
        )
        .await;
    role_alias.assert_status(StatusCode::OK);
    assert_eq!(
        role_alias.value_at(&["data", "user_role"]).as_str(),
        Some("admin")
    );
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}/role", target.id),
        Some(&admin.token),
        json!({ "role": null }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}/role", target.id),
        Some(&admin.token),
        json!({}),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn admin_limits_and_settings_patch_distinguish_absent_null_and_values(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let admin = app.sign_up("admin-limits-patch-admin", "UTC").await;
    let target = app.sign_up("admin-limits-patch-target", "UTC").await;
    promote_super_admin(&app, admin.id).await;

    let quota_set = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/quota", target.id),
            Some(&admin.token),
            json!({ "daily_limit": 7 }),
        )
        .await;
    quota_set.assert_status(StatusCode::OK);
    assert_eq!(
        quota_set
            .value_at(&["data", "ai_extraction", "daily_limit"])
            .as_i64(),
        Some(7)
    );
    let quota_unchanged = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/quota", target.id),
            Some(&admin.token),
            json!({}),
        )
        .await;
    quota_unchanged.assert_status(StatusCode::OK);
    assert_eq!(
        quota_unchanged
            .value_at(&["data", "ai_extraction", "daily_limit"])
            .as_i64(),
        Some(7)
    );
    let quota_cleared = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/quota", target.id),
            Some(&admin.token),
            json!({ "daily_limit": null }),
        )
        .await;
    quota_cleared.assert_status(StatusCode::OK);
    assert_eq!(
        quota_cleared
            .value_at(&["data", "ai_extraction", "daily_limit"])
            .as_i64(),
        Some(10)
    );
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}/quota", target.id),
        Some(&admin.token),
        json!({ "resource_type": null, "daily_limit": 5 }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let content_set = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/content-limits", target.id),
            Some(&admin.token),
            json!({ "resource_type": "subjects", "limit_value": 9 }),
        )
        .await;
    content_set.assert_status(StatusCode::OK);
    assert_limit_value(content_set.value_at(&["data"]), "subjects", 9);
    let content_unchanged = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/content-limits", target.id),
            Some(&admin.token),
            json!({ "resource_type": "subjects" }),
        )
        .await;
    content_unchanged.assert_status(StatusCode::OK);
    assert_limit_value(content_unchanged.value_at(&["data"]), "subjects", 9);
    let content_cleared = app
        .json(
            Method::PATCH,
            &format!("/api/admin/users/{}/content-limits", target.id),
            Some(&admin.token),
            json!({ "resource_type": "subjects", "limit_value": null }),
        )
        .await;
    content_cleared.assert_status(StatusCode::OK);
    assert_limit_value(content_cleared.value_at(&["data"]), "subjects", 6);
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}/content-limits", target.id),
        Some(&admin.token),
        json!({ "limit_value": 3 }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let setting_set = app
        .json(
            Method::PATCH,
            "/api/admin/settings/integration_patch_setting",
            Some(&admin.token),
            json!({
                "value": { "enabled": true },
                "description": "Initial description",
            }),
        )
        .await;
    setting_set.assert_status(StatusCode::OK);
    assert_eq!(
        setting_set
            .value_at(&["setting", "value", "enabled"])
            .as_bool(),
        Some(true)
    );
    assert_eq!(
        setting_set.value_at(&["setting", "description"]).as_str(),
        Some("Initial description")
    );
    let setting_cleared = app
        .json(
            Method::PATCH,
            "/api/admin/settings/integration_patch_setting",
            Some(&admin.token),
            json!({ "description": null }),
        )
        .await;
    setting_cleared.assert_status(StatusCode::OK);
    assert_eq!(
        setting_cleared
            .value_at(&["setting", "value", "enabled"])
            .as_bool(),
        Some(true)
    );
    assert!(
        setting_cleared
            .value_at(&["setting", "description"])
            .is_null()
    );
    let setting_unchanged = app
        .json(
            Method::PATCH,
            "/api/admin/settings/integration_patch_setting",
            Some(&admin.token),
            json!({}),
        )
        .await;
    setting_unchanged.assert_status(StatusCode::OK);
    assert_eq!(
        setting_unchanged
            .value_at(&["setting", "value", "enabled"])
            .as_bool(),
        Some(true)
    );
    assert!(
        setting_unchanged
            .value_at(&["setting", "description"])
            .is_null()
    );
}

#[sqlx::test]
async fn admin_limit_defaults_are_configurable_and_enforced(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let admin = app.sign_up("limit-defaults-admin", "UTC").await;
    let user = app.sign_up("limit-defaults-user", "UTC").await;
    promote_super_admin(&app, admin.id).await;

    // Non-super-admins cannot read the global defaults (403 from ensure_super,
    // since the AdminUser extractor admits any authenticated user).
    app.get("/api/admin/limit-defaults", Some(&user.token))
        .await
        .assert_status(StatusCode::FORBIDDEN);

    // Baseline listing: 7 resources, each configured=null, effective=hardcoded.
    let baseline = app
        .get("/api/admin/limit-defaults", Some(&admin.token))
        .await;
    baseline.assert_status(StatusCode::OK);
    let defaults = baseline.value_at(&["data", "defaults"]);
    let defaults_arr = defaults
        .as_array()
        .unwrap_or_else(|| panic!("defaults is not an array: {defaults}"));
    assert_eq!(defaults_arr.len(), 7);
    assert_eq!(
        configured_for(defaults_arr, "subjects"),
        json!({ "hardcoded": 6, "configured": null, "effective": 6 })
    );
    assert_eq!(
        configured_for(defaults_arr, "ai_extraction"),
        json!({ "hardcoded": 10, "configured": null, "effective": 10 })
    );

    // Negative values are rejected.
    app.json(
        Method::PUT,
        "/api/admin/limit-defaults",
        Some(&admin.token),
        json!({ "subjects": -1 }),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    // Configure a 2-subject global default.
    let set = app
        .json(
            Method::PUT,
            "/api/admin/limit-defaults",
            Some(&admin.token),
            json!({ "subjects": 2 }),
        )
        .await;
    set.assert_status(StatusCode::OK);
    assert_eq!(
        configured_for(
            set.value_at(&["data", "defaults"]).as_array().unwrap(),
            "subjects"
        ),
        json!({ "hardcoded": 6, "configured": 2, "effective": 2 })
    );

    // Enforcement: the user can create 2 subjects, the 3rd is forbidden.
    let _s1 = create_subject(&app, &user).await;
    let _s2 = create_subject(&app, &user).await;
    app.json(
        Method::POST,
        "/api/subjects",
        Some(&user.token),
        json!({ "name": "Third subject", "color": "#3366ff", "icon": "Calculator" }),
    )
    .await
    .assert_status(StatusCode::FORBIDDEN);

    // /api/usage reflects the configured default.
    let usage = app.get("/api/usage", Some(&user.token)).await;
    usage.assert_status(StatusCode::OK);
    assert_limit_value(usage.value_at(&["data", "content_limits"]), "subjects", 2);

    // A per-user override still wins over the configured global default.
    app.json(
        Method::PATCH,
        &format!("/api/admin/users/{}/content-limits", user.id),
        Some(&admin.token),
        json!({ "resource_type": "subjects", "limit_value": 5 }),
    )
    .await
    .assert_status(StatusCode::OK);
    let usage2 = app.get("/api/usage", Some(&user.token)).await;
    assert_limit_value(usage2.value_at(&["data", "content_limits"]), "subjects", 5);

    // Resetting the configured default restores the hardcoded baseline (6).
    let reset = app
        .json(
            Method::PUT,
            "/api/admin/limit-defaults",
            Some(&admin.token),
            json!({}),
        )
        .await;
    reset.assert_status(StatusCode::OK);
    assert_eq!(
        configured_for(
            reset.value_at(&["data", "defaults"]).as_array().unwrap(),
            "subjects"
        ),
        json!({ "hardcoded": 6, "configured": null, "effective": 6 })
    );
}

/// Extract `{hardcoded, configured, effective}` for a resource from the
/// limit-defaults listing array.
fn configured_for(defaults: &[Value], resource_type: &str) -> Value {
    let entry = defaults
        .iter()
        .find(|d| d.get("resource_type").and_then(Value::as_str) == Some(resource_type))
        .unwrap_or_else(|| panic!("missing {resource_type} in defaults {defaults:?}"));
    json!({
        "hardcoded": entry.get("hardcoded").cloned().unwrap_or(Value::Null),
        "configured": entry.get("configured").cloned().unwrap_or(Value::Null),
        "effective": entry.get("effective").cloned().unwrap_or(Value::Null),
    })
}

fn assert_limit_value(data: &Value, resource_type: &str, expected_limit: i64) {
    let limit = data
        .as_array()
        .and_then(|limits| {
            limits.iter().find(|limit| {
                limit.get("resource_type").and_then(Value::as_str) == Some(resource_type)
            })
        })
        .unwrap_or_else(|| panic!("missing {resource_type} limit in {data}"));
    assert_eq!(
        limit.get("limit").and_then(Value::as_i64),
        Some(expected_limit),
        "limit response: {data}"
    );
}

#[sqlx::test]
async fn review_schedule_same_day_attempt_does_not_advance_sm2(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("review-schedule", "Asia/Shanghai").await;
    let subject_id = create_subject(&app, &session).await;
    let problem_id = create_problem(&app, &session, subject_id).await;

    for answer in ["first", "same-day-second"] {
        app.json(
            Method::POST,
            "/api/attempts",
            Some(&session.token),
            json!({
                "problem_id": problem_id,
                "submitted_answer": { "text": answer },
                "is_correct": true,
                "selected_status": "mastered",
            }),
        )
        .await
        .assert_status(StatusCode::CREATED);
    }

    let row = sqlx::query_as::<_, (i32, f64, i32, bool, bool)>(
        r#"
        select
          repetition_number,
          ease_factor,
          interval_days,
          last_reviewed_at is not null,
          next_review_at = ((timezone('Asia/Shanghai', now())::date + 1)::timestamp at time zone 'Asia/Shanghai')
        from review_schedule
        where user_id = $1 and problem_id = $2
        "#,
    )
    .bind(session.id)
    .bind(problem_id)
    .fetch_one(&app.pool)
    .await
    .expect("review schedule row");

    assert_eq!(row.0, 1, "same-day review must not increment repetition");
    assert!((row.1 - 2.6).abs() < f64::EPSILON, "unexpected ease factor");
    assert_eq!(row.2, 1, "same-day review must keep first interval");
    assert!(row.3, "last_reviewed_at should be populated");
    assert!(row.4, "next_review_at should be next local midnight");
}

#[sqlx::test]
async fn upload_policy_rejects_invalid_mime_on_file_and_qr_uploads(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;
    let session = app.sign_up("upload-policy", "UTC").await;
    let subject_id = create_subject(&app, &session).await;
    let problem_id = create_problem(&app, &session, subject_id).await;

    let boundary = format!("boundary-{}", Uuid::new_v4());
    app.multipart(
        "/api/files/upload",
        Some(&session.token),
        &boundary,
        multipart_problem_upload(
            &boundary,
            problem_id,
            "notes.txt",
            "text/plain",
            "plain text is not an allowed problem upload",
        ),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let qr_session = app
        .json(
            Method::POST,
            "/api/qr-sessions",
            Some(&session.token),
            json!({}),
        )
        .await;
    qr_session.assert_status(StatusCode::OK);
    let session_id = qr_session.uuid_at(&["data", "sessionId"]);
    let token = qr_session.string_at(&["data", "token"]);
    let qr_boundary = format!("boundary-{}", Uuid::new_v4());
    app.multipart(
        &format!("/api/qr-upload/{session_id}?token={token}"),
        None,
        &qr_boundary,
        multipart_file(
            &qr_boundary,
            "notes.txt",
            "text/plain",
            "plain text is not an allowed QR upload",
        ),
    )
    .await
    .assert_status(StatusCode::BAD_REQUEST);

    let status = app
        .get(
            &format!("/api/qr-sessions/{session_id}/status"),
            Some(&session.token),
        )
        .await;
    status.assert_status(StatusCode::OK);
    assert_eq!(
        status.value_at(&["data", "status"]).as_str(),
        Some("pending")
    );
    assert!(status.value_at(&["data", "filePath"]).is_null());
}

#[sqlx::test]
async fn sign_up_blocked_when_registration_disabled(pool: PgPool) {
    let app = TestApp::from_pool(pool).await;

    // Registration is open by default (the seed migration sets enabled=true).
    let status = app.get("/api/registration-status", None).await;
    status.assert_ok();
    assert_eq!(status.value_at(&["data", "enabled"]), &Value::Bool(true));

    // Close registration by upserting the admin setting directly.
    sqlx::query(
        r#"insert into admin_settings (key, value, description)
           values ('user_registration', '{"enabled": false}'::jsonb, 'test')
           on conflict (key) do update set value = excluded.value"#,
    )
    .execute(&app.pool)
    .await
    .unwrap();

    // The public status endpoint reflects the closed state.
    let closed = app.get("/api/registration-status", None).await;
    closed.assert_ok();
    assert_eq!(closed.value_at(&["data", "enabled"]), &Value::Bool(false));

    // Sign-up is rejected while registration is closed.
    let blocked = app
        .json(
            Method::POST,
            "/api/auth/sign-up",
            None,
            json!({
                "email": format!("closed-{}@example.test", Uuid::new_v4()),
                "password": "password123",
                "timezone": "UTC",
            }),
        )
        .await;
    blocked.assert_status(StatusCode::FORBIDDEN);

    // Re-enable registration; the status flips back and sign-up succeeds.
    sqlx::query(
        r#"insert into admin_settings (key, value, description)
           values ('user_registration', '{"enabled": true}'::jsonb, 'test')
           on conflict (key) do update set value = excluded.value"#,
    )
    .execute(&app.pool)
    .await
    .unwrap();
    let reopened = app.get("/api/registration-status", None).await;
    assert_eq!(reopened.value_at(&["data", "enabled"]), &Value::Bool(true));
    app.sign_up("reopened", "UTC").await;
}
