use std::{
    path::{Component, Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use axum::body::Bytes;
use sqlx::PgPool;
use tokio::fs;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

pub const PROBLEM_UPLOADS_BUCKET: &str = "problem-uploads";
pub const AVATARS_BUCKET: &str = "avatars";
pub const MAX_PROBLEM_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;

const PROBLEM_UPLOAD_CONTENT_TYPES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "application/pdf",
];
const AVATAR_CONTENT_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp", "image/gif"];

#[derive(Debug)]
pub struct ValidatedUpload {
    pub file_name: String,
    pub content_type: String,
    pub bytes: Bytes,
}

pub fn validate_user_storage_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    validate_relative_path(path)
        && parts.len() >= 3
        && parts[0] == "user"
        && Uuid::parse_str(parts[1]).is_ok()
}

pub fn validate_avatar_storage_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    validate_relative_path(path)
        && parts.len() == 2
        && Uuid::parse_str(parts[0]).is_ok()
        && parts[1].starts_with("avatar.")
}

pub fn validate_problem_upload_content_type(content_type: &str) -> bool {
    PROBLEM_UPLOAD_CONTENT_TYPES.contains(&content_type)
}

pub fn validate_avatar_content_type(content_type: &str) -> bool {
    AVATAR_CONTENT_TYPES.contains(&content_type)
}

pub fn validate_problem_upload(
    file_name: Option<&str>,
    content_type: Option<&str>,
    bytes: Bytes,
) -> AppResult<ValidatedUpload> {
    let content_type = content_type
        .unwrap_or("application/octet-stream")
        .to_owned();
    if !validate_problem_upload_content_type(&content_type) {
        return Err(AppError::BadRequest(
            "Unsupported file content type".to_owned(),
        ));
    }
    if bytes.len() > MAX_PROBLEM_UPLOAD_BYTES {
        return Err(AppError::BadRequest("File exceeds 10MB limit".to_owned()));
    }

    Ok(ValidatedUpload {
        file_name: safe_file_name(file_name.unwrap_or("upload")),
        content_type,
        bytes,
    })
}

pub fn validate_avatar_upload(
    file_name: Option<&str>,
    content_type: Option<&str>,
    bytes: Bytes,
) -> AppResult<ValidatedUpload> {
    let content_type = content_type
        .unwrap_or("application/octet-stream")
        .to_owned();
    if !validate_avatar_content_type(&content_type) {
        return Err(AppError::BadRequest(
            "Unsupported avatar content type".to_owned(),
        ));
    }
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(AppError::BadRequest("Avatar exceeds 2MB limit".to_owned()));
    }

    Ok(ValidatedUpload {
        file_name: safe_file_name(file_name.unwrap_or("avatar")),
        content_type,
        bytes,
    })
}

pub fn avatar_extension(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "jpg",
    }
}

pub fn path_owned_by_user(path: &str, user_id: Uuid) -> bool {
    path.starts_with(&format!("user/{user_id}/"))
}

pub fn all_assets_owned_by_user(
    user_id: Uuid,
    assets: &[crate::models::Asset],
    solution_assets: &[crate::models::Asset],
) -> bool {
    assets.iter().chain(solution_assets.iter()).all(|asset| {
        validate_user_storage_path(&asset.path) && path_owned_by_user(&asset.path, user_id)
    })
}

pub fn file_url(bucket: &str, path: &str) -> String {
    match bucket {
        AVATARS_BUCKET => format!("/api/files/avatars/{}", encode_path_segments(path)),
        _ => format!("/api/files/{}", encode_path_segments(path)),
    }
}

pub async fn upload_object(
    state: &AppState,
    bucket: &str,
    path: &str,
    _content_type: &str,
    bytes: Bytes,
    upsert: bool,
) -> AppResult<()> {
    let object = local_object_path(state, bucket, path)?;
    if !upsert && fs::try_exists(&object).await.unwrap_or(false) {
        return Err(AppError::Conflict("File already exists".to_owned()));
    }
    if let Some(parent) = object.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|err| AppError::External(format!("Storage directory create failed: {err}")))?;
    }
    let temp_object = temporary_object_path(&object)?;
    fs::write(&temp_object, bytes)
        .await
        .map_err(|err| AppError::External(format!("Storage write failed: {err}")))?;
    if let Err(err) = scan_object(state, &temp_object) {
        let _ = fs::remove_file(&temp_object).await;
        return Err(err);
    }
    fs::rename(&temp_object, &object)
        .await
        .map_err(|err| AppError::External(format!("Storage commit failed: {err}")))?;
    Ok(())
}

pub async fn read_object(state: &AppState, bucket: &str, path: &str) -> AppResult<(Bytes, String)> {
    let object = local_object_path(state, bucket, path)?;
    let bytes = fs::read(&object).await.map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => AppError::NotFound("Not found".to_owned()),
        _ => AppError::External(format!("Storage read failed: {err}")),
    })?;
    let content_type = mime_guess::from_path(&object)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();
    Ok((Bytes::from(bytes), content_type))
}

pub async fn delete_object(state: &AppState, bucket: &str, path: &str) -> AppResult<()> {
    let object = local_object_path(state, bucket, path)?;
    match fs::remove_file(&object).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::External(format!("Storage delete failed: {err}"))),
    }
}

/// A problem-upload draft directory: `problem-uploads/user/{user_id}/problems/{problem_id}/`.
/// Held by the background reaper to decide whether a draft's files are orphaned
/// (no matching `problems` row) and stale enough to reclaim.
#[derive(Debug)]
pub struct DraftDir {
    pub user_id: Uuid,
    pub problem_id: Uuid,
    pub modified: SystemTime,
}

/// Delete the entire draft directory for one problem UUID — the files uploaded
/// while a problem form was open but never submitted. Idempotent: a missing
/// directory is not an error. Only ever touches `user/{user_id}/problems/{problem_id}`,
/// never a persisted problem's assets.
pub async fn delete_problem_draft_dir(
    state: &AppState,
    user_id: Uuid,
    problem_id: Uuid,
) -> AppResult<()> {
    let prefix = format!("user/{user_id}/problems/{problem_id}");
    // Defensive: confirm the prefix is a valid, user-owned path before joining
    // it onto the storage root. The uuids are typed, so this can only fail if
    // the layout assumption changes — fail closed rather than delete the wrong tree.
    if !validate_user_storage_path(&format!("{prefix}/problem")) {
        return Err(AppError::Forbidden);
    }
    let dir = state
        .config
        .local_storage_root
        .join(PROBLEM_UPLOADS_BUCKET)
        .join(&prefix);
    match fs::remove_dir_all(&dir).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::External(format!(
            "Storage draft cleanup failed: {err}"
        ))),
    }
}

/// Enumerate every `user/{user_id}/problems/{problem_id}/` draft directory on
/// disk, with its last-modified time. The reaper pairs this list with a DB
/// existence check to reclaim orphans. Filesystem-only; never touches the DB.
pub async fn list_problem_draft_dirs(state: &AppState) -> AppResult<Vec<DraftDir>> {
    let root = state
        .config
        .local_storage_root
        .join(PROBLEM_UPLOADS_BUCKET)
        .join("user");
    let mut out = Vec::new();
    let mut user_dirs = match fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(err) => {
            return Err(AppError::External(format!(
                "Storage draft scan failed: {err}"
            )));
        }
    };
    while let Some(user_entry) = user_dirs
        .next_entry()
        .await
        .map_err(|err| AppError::External(format!("Storage draft scan failed: {err}")))?
    {
        let Some(user_name) = user_entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(user_id) = Uuid::parse_str(&user_name) else {
            continue;
        };
        let mut problem_dirs = match fs::read_dir(user_entry.path().join("problems")).await {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        while let Some(problem_entry) = problem_dirs
            .next_entry()
            .await
            .map_err(|err| AppError::External(format!("Storage draft scan failed: {err}")))?
        {
            let Some(problem_name) = problem_entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(problem_id) = Uuid::parse_str(&problem_name) else {
                continue;
            };
            let modified = problem_entry
                .metadata()
                .await
                .ok()
                .and_then(|meta| meta.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push(DraftDir {
                user_id,
                problem_id,
                modified,
            });
        }
    }
    Ok(out)
}

pub async fn find_problem_by_asset(pool: &PgPool, path: &str) -> AppResult<Option<Uuid>> {
    let problem_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        select id
        from problems
        where exists (
          select 1 from jsonb_array_elements(coalesce(assets, '[]'::jsonb)) item
          where item->>'path' = $1
        )
        or exists (
          select 1 from jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) item
          where item->>'path' = $1
        )
        limit 1
        "#,
    )
    .bind(path)
    .fetch_optional(pool)
    .await?;
    Ok(problem_id)
}

pub async fn can_view_problem(
    pool: &PgPool,
    problem_id: Uuid,
    user_id: Option<Uuid>,
    user_email: Option<&str>,
) -> AppResult<bool> {
    let owner = sqlx::query_scalar::<_, Option<Uuid>>("select user_id from problems where id = $1")
        .bind(problem_id)
        .fetch_optional(pool)
        .await?
        .flatten();
    let Some(owner_id) = owner else {
        return Ok(false);
    };
    if user_id == Some(owner_id) {
        return Ok(true);
    }

    let public = sqlx::query_scalar::<_, bool>(
        r#"
        select exists(
          select 1
          from problem_set_problems psp
          join problem_sets ps on ps.id = psp.problem_set_id
          where psp.problem_id = $1 and ps.sharing_level = 'public'
        ) or exists(
          select 1
          from problems p
          join problem_sets ps on ps.subject_id = p.subject_id and ps.user_id = p.user_id
          where p.id = $1 and ps.sharing_level = 'public' and ps.is_smart = true
        )
        "#,
    )
    .bind(problem_id)
    .fetch_one(pool)
    .await?;
    if public {
        return Ok(true);
    }

    if let (Some(_user_id), Some(email)) = (user_id, user_email) {
        let limited = sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
              select 1
              from problem_set_problems psp
              join problem_sets ps on ps.id = psp.problem_set_id
              join problem_set_shares pss on pss.problem_set_id = ps.id
              where psp.problem_id = $1
                and ps.sharing_level = 'limited'
                and lower(pss.shared_with_email) = lower($2)
            )
            "#,
        )
        .bind(problem_id)
        .bind(email)
        .fetch_one(pool)
        .await?;
        if limited {
            return Ok(true);
        }
    }

    Ok(false)
}

pub async fn user_owns_problem_with_asset(
    pool: &PgPool,
    user_id: Uuid,
    path: &str,
) -> AppResult<bool> {
    let owns = sqlx::query_scalar::<_, bool>(
        r#"
        select exists(
          select 1 from problems
          where user_id = $1 and (
            exists (
              select 1 from jsonb_array_elements(coalesce(assets, '[]'::jsonb)) item
              where item->>'path' = $2
            )
            or exists (
              select 1 from jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) item
              where item->>'path' = $2
            )
          )
        )
        "#,
    )
    .bind(user_id)
    .bind(path)
    .fetch_one(pool)
    .await?;
    Ok(owns)
}

fn local_object_path(state: &AppState, bucket: &str, path: &str) -> AppResult<PathBuf> {
    let valid = match bucket {
        PROBLEM_UPLOADS_BUCKET => validate_user_storage_path(path),
        AVATARS_BUCKET => validate_avatar_storage_path(path),
        _ => false,
    };
    if !valid {
        return Err(AppError::Forbidden);
    }
    Ok(state.config.local_storage_root.join(bucket).join(path))
}

fn temporary_object_path(object: &Path) -> AppResult<PathBuf> {
    let file_name = object
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AppError::Internal("Storage object missing file name".to_owned()))?;
    Ok(object.with_file_name(format!(".{file_name}.scan-{}", Uuid::new_v4())))
}

fn scan_object(state: &AppState, object: &Path) -> AppResult<()> {
    let Some(command) = state.config.local_storage_scan_command.as_deref() else {
        return Ok(());
    };
    let mut parts = command.split_whitespace();
    let executable = parts
        .next()
        .ok_or_else(|| AppError::Configuration("LOCAL_STORAGE_SCAN_COMMAND is empty".to_owned()))?;
    let status = Command::new(executable)
        .args(parts)
        .arg(object)
        .status()
        .map_err(|err| AppError::External(format!("Storage scan failed to start: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "File failed content safety scan".to_owned(),
        ))
    }
}

fn validate_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains('~')
    {
        return false;
    }
    Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

pub fn safe_file_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized
        .trim_matches('.')
        .chars()
        .take(120)
        .collect::<String>();
    if trimmed.is_empty() {
        "upload".to_owned()
    } else {
        trimmed
    }
}

fn encode_path_segments(path: &str) -> String {
    path.split('/')
        .map(urlencoding::encode)
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Asset;

    #[test]
    fn validates_user_storage_paths_and_rejects_traversal() {
        let user_id = Uuid::new_v4();
        let valid = format!("user/{user_id}/problems/problem-1/problem/image.png");

        assert!(validate_user_storage_path(&valid));
        assert!(!validate_user_storage_path(""));
        assert!(!validate_user_storage_path("/user/id/file.png"));
        assert!(!validate_user_storage_path("../file.png"));
        assert!(!validate_user_storage_path(&format!(
            "user/{user_id}/../file.png"
        )));
        assert!(!validate_user_storage_path(&format!(
            "user/{user_id}\\file.png"
        )));
        assert!(!validate_user_storage_path("user/not-a-uuid/file.png"));
    }

    #[test]
    fn validates_avatar_paths_and_rejects_non_avatar_files() {
        let user_id = Uuid::new_v4();

        assert!(validate_avatar_storage_path(&format!(
            "{user_id}/avatar.png"
        )));
        assert!(validate_avatar_storage_path(&format!(
            "{user_id}/avatar.webp"
        )));
        assert!(!validate_avatar_storage_path(&format!(
            "{user_id}/profile.png"
        )));
        assert!(!validate_avatar_storage_path(&format!(
            "{user_id}/nested/avatar.png"
        )));
        assert!(!validate_avatar_storage_path("not-a-uuid/avatar.png"));
        assert!(!validate_avatar_storage_path("../avatar.png"));
    }

    #[test]
    fn checks_asset_ownership_for_problem_uploads() {
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let owned = Asset {
            path: format!("user/{owner}/problems/problem-1/problem/image.png"),
            kind: Some("image".to_owned()),
        };
        let foreign = Asset {
            path: format!("user/{other}/problems/problem-1/problem/image.png"),
            kind: Some("image".to_owned()),
        };

        assert!(path_owned_by_user(&owned.path, owner));
        assert!(!path_owned_by_user(&foreign.path, owner));
        assert!(all_assets_owned_by_user(
            owner,
            std::slice::from_ref(&owned),
            &[]
        ));
        assert!(!all_assets_owned_by_user(owner, &[owned, foreign], &[]));
    }

    #[test]
    fn builds_file_urls_with_encoded_segments() {
        let user_id = Uuid::new_v4();
        let avatar_path = format!("{user_id}/avatar.png");
        let problem_path = format!("user/{user_id}/problems/p 1/problem/a+b.png");

        assert_eq!(
            file_url(AVATARS_BUCKET, &avatar_path),
            format!("/api/files/avatars/{user_id}/avatar.png")
        );
        assert_eq!(
            file_url(PROBLEM_UPLOADS_BUCKET, &problem_path),
            format!("/api/files/user/{user_id}/problems/p%201/problem/a%2Bb.png")
        );
    }

    #[test]
    fn validates_upload_content_types() {
        assert!(validate_problem_upload_content_type("image/jpeg"));
        assert!(validate_problem_upload_content_type("application/pdf"));
        assert!(!validate_problem_upload_content_type("text/plain"));
        assert!(!validate_problem_upload_content_type("image/svg+xml"));

        assert!(validate_avatar_content_type("image/webp"));
        assert!(!validate_avatar_content_type("application/pdf"));
    }

    #[test]
    fn validates_problem_upload_payloads() {
        let upload = validate_problem_upload(
            Some("../my image?.jpg"),
            Some("image/jpeg"),
            Bytes::from_static(b"image"),
        )
        .expect("valid problem upload");

        assert_eq!(upload.file_name, "_my_image_.jpg");
        assert_eq!(upload.content_type, "image/jpeg");
        assert_eq!(upload.bytes, Bytes::from_static(b"image"));

        assert!(matches!(
            validate_problem_upload(
                Some("notes.txt"),
                Some("text/plain"),
                Bytes::from_static(b"text")
            ),
            Err(AppError::BadRequest(message)) if message == "Unsupported file content type"
        ));
        assert!(matches!(
            validate_problem_upload(
                Some("large.pdf"),
                Some("application/pdf"),
                Bytes::from(vec![0; MAX_PROBLEM_UPLOAD_BYTES + 1])
            ),
            Err(AppError::BadRequest(message)) if message == "File exceeds 10MB limit"
        ));
    }

    #[test]
    fn validates_avatar_upload_payloads() {
        let upload = validate_avatar_upload(
            Some("avatar.webp"),
            Some("image/webp"),
            Bytes::from_static(b"avatar"),
        )
        .expect("valid avatar upload");

        assert_eq!(upload.file_name, "avatar.webp");
        assert_eq!(upload.content_type, "image/webp");
        assert_eq!(avatar_extension(&upload.content_type), "webp");

        assert!(matches!(
            validate_avatar_upload(
                Some("avatar.pdf"),
                Some("application/pdf"),
                Bytes::from_static(b"pdf")
            ),
            Err(AppError::BadRequest(message)) if message == "Unsupported avatar content type"
        ));
        assert!(matches!(
            validate_avatar_upload(
                Some("large.png"),
                Some("image/png"),
                Bytes::from(vec![0; MAX_AVATAR_BYTES + 1])
            ),
            Err(AppError::BadRequest(message)) if message == "Avatar exceeds 2MB limit"
        ));
    }
}
