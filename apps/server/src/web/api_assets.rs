use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::{AdminApiError, authenticate, require_csrf};
use crate::{AppState, AuditDraft};

const PUBLIC_BASE: &str = "/user-assets";

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list_assets).delete(delete_asset))
        .route("/assets/folders", post(create_folder))
        .route("/assets/files", post(upload_file))
}

#[derive(Deserialize)]
struct PathQuery {
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
struct UploadQuery {
    #[serde(default)]
    path: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateFolderRequest {
    #[serde(default)]
    path: String,
    name: String,
}

#[derive(Serialize)]
struct AssetListResponse {
    schema: &'static str,
    path: String,
    public_base: &'static str,
    entries: Vec<AssetEntry>,
}

#[derive(Serialize)]
struct AssetEntry {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
    modified_at_ms: u64,
    media_type: &'static str,
}

async fn list_assets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PathQuery>,
) -> Result<Json<AssetListResponse>, AdminApiError> {
    authenticate(&state, &headers)?;
    let relative = safe_relative_path(&query.path)?;
    let display_path = relative_path_string(&relative);
    let root = state.assets_dir().to_owned();
    let entries = tokio::task::spawn_blocking(move || list_directory(&root, &relative))
        .await
        .map_err(|_| AdminApiError::internal())??;
    Ok(Json(AssetListResponse {
        schema: "admin_asset_list.v1",
        path: display_path,
        public_base: PUBLIC_BASE,
        entries,
    }))
}

async fn create_folder(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateFolderRequest>,
) -> Result<(StatusCode, Json<AssetEntry>), AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    let parent = safe_relative_path(&request.path)?;
    let name = safe_name(&request.name)?;
    let relative = parent.join(name);
    let root = state.assets_dir().to_owned();
    let task_relative = relative.clone();
    let entry = tokio::task::spawn_blocking(move || {
        ensure_directory(&root, &parent, false)?;
        let target = root.join(&task_relative);
        fs::create_dir(&target).map_err(map_create_error)?;
        entry_from_path(&target, &task_relative)
    })
    .await
    .map_err(|_| AdminApiError::internal())??;
    record_asset_audit(
        &state,
        authenticated.user.id().as_str(),
        "admin.asset.folder_created",
        &relative_path_string(&relative),
        "资源文件夹已创建",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(entry)))
}

async fn upload_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> Result<(StatusCode, Json<AssetEntry>), AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    if body.is_empty() {
        return Err(AdminApiError::bad_request(
            "asset.empty_file",
            "不能上传空文件",
        ));
    }
    let parent = safe_relative_path(&query.path)?;
    let name = safe_name(&query.name)?;
    let relative = parent.join(name);
    let root = state.assets_dir().to_owned();
    let task_relative = relative.clone();
    let entry = tokio::task::spawn_blocking(move || {
        ensure_directory(&root, &parent, true)?;
        let target = root.join(&task_relative);
        reject_symlink(&target)?;
        fs::write(&target, &body).map_err(|_| AdminApiError::internal())?;
        entry_from_path(&target, &task_relative)
    })
    .await
    .map_err(|_| AdminApiError::internal())??;
    record_asset_audit(
        &state,
        authenticated.user.id().as_str(),
        "admin.asset.uploaded",
        &relative_path_string(&relative),
        "资源文件已上传",
    )
    .await?;
    Ok((StatusCode::CREATED, Json(entry)))
}

async fn delete_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PathQuery>,
) -> Result<StatusCode, AdminApiError> {
    let authenticated = authenticate(&state, &headers)?;
    require_csrf(&headers, &authenticated)?;
    let relative = safe_relative_path(&query.path)?;
    if relative.as_os_str().is_empty() {
        return Err(AdminApiError::bad_request(
            "asset.root_protected",
            "不能删除资源根目录",
        ));
    }
    let display_path = relative_path_string(&relative);
    let root = state.assets_dir().to_owned();
    tokio::task::spawn_blocking(move || {
        ensure_safe_ancestors(&root, &relative)?;
        let target = root.join(relative);
        let metadata = fs::symlink_metadata(&target).map_err(map_not_found)?;
        if metadata.file_type().is_symlink() {
            return Err(AdminApiError::bad_request(
                "asset.symlink_unsupported",
                "不支持符号链接",
            ));
        }
        if metadata.is_dir() {
            fs::remove_dir_all(target).map_err(|_| AdminApiError::internal())
        } else {
            fs::remove_file(target).map_err(|_| AdminApiError::internal())
        }
    })
    .await
    .map_err(|_| AdminApiError::internal())??;
    record_asset_audit(
        &state,
        authenticated.user.id().as_str(),
        "admin.asset.deleted",
        &display_path,
        "资源已删除",
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn record_asset_audit(
    state: &AppState,
    actor_id: &str,
    action: &'static str,
    target_id: &str,
    detail: &'static str,
) -> Result<(), AdminApiError> {
    state
        .record_audit(AuditDraft {
            severity: "info",
            category: "admin",
            action,
            actor_id: Some(actor_id.to_owned()),
            target_type: "asset",
            target_id: Some(target_id.to_owned()),
            outcome: "success",
            detail: detail.to_owned(),
        })
        .await?;
    Ok(())
}

fn safe_relative_path(value: &str) -> Result<PathBuf, AdminApiError> {
    let value = value.trim().trim_matches('/');
    if value.is_empty() {
        return Ok(PathBuf::new());
    }
    if value.contains('\\') {
        return Err(invalid_path());
    }
    let path = Path::new(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_path());
    }
    Ok(path.to_owned())
}

fn safe_name(value: &str) -> Result<&OsStr, AdminApiError> {
    let value = value.trim();
    let path = Path::new(value);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) if !value.contains('\\') => Ok(name),
        _ => Err(invalid_path()),
    }
}

fn invalid_path() -> AdminApiError {
    AdminApiError::bad_request("asset.invalid_path", "资源路径无效")
}

fn ensure_directory(root: &Path, relative: &Path, create: bool) -> Result<(), AdminApiError> {
    ensure_safe_ancestors(root, relative)?;
    let directory = root.join(relative);
    if create {
        fs::create_dir_all(&directory).map_err(|_| AdminApiError::internal())?;
    }
    let metadata = fs::symlink_metadata(&directory).map_err(map_not_found)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid_path());
    }
    Ok(())
}

fn ensure_safe_ancestors(root: &Path, relative: &Path) -> Result<(), AdminApiError> {
    let mut current = root.to_owned();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        if index + 1 == component_count {
            break;
        }
        current.push(component.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(invalid_path());
            }
        }
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), AdminApiError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_dir() => {
            Err(invalid_path())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AdminApiError::internal()),
    }
}

fn list_directory(root: &Path, relative: &Path) -> Result<Vec<AssetEntry>, AdminApiError> {
    ensure_directory(root, relative, false)?;
    let directory = root.join(relative);
    let mut entries = fs::read_dir(directory)
        .map_err(map_not_found)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let child_relative = relative.join(&name);
            entry_from_path(&entry.path(), &child_relative).ok()
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        (left.kind != "folder", left.name.to_lowercase())
            .cmp(&(right.kind != "folder", right.name.to_lowercase()))
    });
    Ok(entries)
}

fn entry_from_path(path: &Path, relative: &Path) -> Result<AssetEntry, AdminApiError> {
    let metadata = fs::symlink_metadata(path).map_err(map_not_found)?;
    if metadata.file_type().is_symlink() {
        return Err(invalid_path());
    }
    let name = relative
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(invalid_path)?
        .to_owned();
    let kind = if metadata.is_dir() { "folder" } else { "file" };
    Ok(AssetEntry {
        media_type: if metadata.is_dir() {
            "folder"
        } else {
            media_type(relative)
        },
        name,
        path: relative_path_string(relative),
        kind,
        size: if metadata.is_file() {
            metadata.len()
        } else {
            0
        },
        modified_at_ms: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_millis() as u64),
    })
}

fn media_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "avif" => "image",
        "mp3" | "ogg" | "wav" | "m4a" | "aac" | "flac" | "opus" => "audio",
        "mp4" | "webm" | "mov" => "video",
        "json" | "txt" | "css" | "js" | "xml" | "md" => "text",
        _ => "binary",
    }
}

fn relative_path_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

fn map_not_found(error: std::io::Error) -> AdminApiError {
    if error.kind() == std::io::ErrorKind::NotFound {
        AdminApiError::not_found("asset.not_found", "资源不存在")
    } else {
        AdminApiError::internal()
    }
}

fn map_create_error(error: std::io::Error) -> AdminApiError {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        AdminApiError::conflict("asset.already_exists", "同名资源已存在")
    } else {
        AdminApiError::internal()
    }
}

#[cfg(test)]
mod tests {
    use super::{media_type, safe_name, safe_relative_path};

    #[test]
    fn paths_stay_inside_the_asset_root() {
        assert!(safe_relative_path("characters/hero").is_ok());
        assert!(safe_relative_path("../private").is_err());
        assert!(safe_relative_path("characters\\hero").is_err());
        assert!(safe_name("hero.png").is_ok());
        assert!(safe_name("nested/hero.png").is_err());
    }

    #[test]
    fn classifies_common_asset_extensions() {
        assert_eq!(media_type(Path::new("hero.PNG")), "image");
        assert_eq!(media_type(Path::new("lobby.ogg")), "audio");
        assert_eq!(media_type(Path::new("model.bin")), "binary");
    }

    use std::path::Path;
}
