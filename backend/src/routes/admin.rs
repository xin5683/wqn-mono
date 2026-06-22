use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, patch},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::AdminUser,
    dto::admin::{AdminUserDto, AdminUsersResponse},
    error::{AppError, AppResult},
    response,
    services::{
        accounts,
        admin::{
            self as admin_service, AdminQuery, PatchAdminContentLimit, PatchAdminQuota,
            PatchAdminRole, PatchAdminSetting, PatchAdminUser,
        },
        limits_config::{self, LimitDefaults},
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list_users))
        .route(
            "/admin/users/{id}",
            get(get_user).patch(update_user).delete(delete_user),
        )
        .route("/admin/users/{id}/role", patch(update_role))
        .route("/admin/users/{id}/toggle-active", patch(toggle_active))
        .route(
            "/admin/users/{id}/content-statistics",
            get(get_user_content_statistics),
        )
        .route(
            "/admin/users/{id}/storage-usage",
            get(get_user_storage_usage),
        )
        .route(
            "/admin/users/{id}/quota",
            get(get_user_quota).patch(set_user_quota),
        )
        .route(
            "/admin/users/{id}/content-limits",
            get(get_user_content_limits).patch(set_user_content_limit),
        )
        .route("/admin/settings", get(list_settings))
        .route("/admin/settings/{key}", patch(update_setting))
        .route(
            "/admin/limit-defaults",
            get(list_limit_defaults).put(put_limit_defaults),
        )
        .route("/admin/statistics", get(statistics))
        .route("/admin/activity", get(activity))
}

async fn list_users(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<AdminQuery>,
) -> AppResult<Json<AdminUsersResponse>> {
    ensure_super(&admin)?;
    Ok(Json(admin_service::list_users(&state.pool, query).await?))
}

async fn get_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<AdminUserDto>>> {
    ensure_super(&admin)?;
    accounts::success_or_not_found(accounts::admin_user(&state.pool, id).await?)
}

async fn update_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchAdminUser>,
) -> AppResult<Json<response::ApiSuccess<AdminUserDto>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::update_user(&state.pool, admin.user.id, id, body).await?,
    ))
}

async fn delete_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    admin_service::delete_user(&state.pool, admin.user.id, id).await?;
    Ok(response::empty_ok())
}

async fn update_role(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchAdminRole>,
) -> AppResult<Json<response::ApiSuccess<AdminUserDto>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::update_role(&state.pool, admin.user.id, id, body).await?,
    ))
}

async fn toggle_active(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<AdminUserDto>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::toggle_active(&state.pool, admin.user.id, id).await?,
    ))
}

async fn get_user_content_statistics(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::user_content_statistics(&state.pool, id).await?,
    ))
}

async fn get_user_storage_usage(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::user_storage_usage(&state.pool, id).await?,
    ))
}

async fn get_user_quota(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::user_quota(&state.pool, id).await?,
    ))
}

async fn set_user_quota(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchAdminQuota>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::set_user_quota(&state.pool, id, body).await?,
    ))
}

async fn get_user_content_limits(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::user_content_limits(&state.pool, id).await?,
    ))
}

async fn set_user_content_limit(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchAdminContentLimit>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        admin_service::set_user_content_limit(&state.pool, id, body).await?,
    ))
}

async fn list_settings(State(state): State<AppState>, admin: AdminUser) -> AppResult<Json<Value>> {
    ensure_super(&admin)?;
    Ok(Json(admin_service::list_settings(&state.pool).await?))
}

async fn update_setting(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(key): Path<String>,
    Json(body): Json<PatchAdminSetting>,
) -> AppResult<Json<Value>> {
    ensure_super(&admin)?;
    Ok(Json(
        admin_service::update_setting(&state.pool, admin.user.id, key, body).await?,
    ))
}

async fn statistics(State(state): State<AppState>, admin: AdminUser) -> AppResult<Json<Value>> {
    ensure_super(&admin)?;
    Ok(Json(admin_service::statistics(&state.pool).await?))
}

async fn activity(
    State(state): State<AppState>,
    admin: AdminUser,
    Query(query): Query<AdminQuery>,
) -> AppResult<Json<Value>> {
    ensure_super(&admin)?;
    Ok(Json(admin_service::activity(&state.pool, query).await?))
}

async fn list_limit_defaults(
    State(state): State<AppState>,
    admin: AdminUser,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        limits_config::list_limit_defaults(&state.pool).await?,
    ))
}

async fn put_limit_defaults(
    State(state): State<AppState>,
    admin: AdminUser,
    Json(body): Json<LimitDefaults>,
) -> AppResult<Json<response::ApiSuccess<Value>>> {
    ensure_super(&admin)?;
    Ok(response::success(
        limits_config::set_limit_defaults(&state.pool, admin.user.id, body).await?,
    ))
}

fn ensure_super(admin: &AdminUser) -> AppResult<()> {
    if accounts::is_super_admin_role(&admin.role) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
