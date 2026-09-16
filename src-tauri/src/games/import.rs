use crate::{categories::Category, deep_link::parser::validate_game_id, games::catalog};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, Runtime};
use zip::ZipArchive;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportGamesArchiveSummary {
    pub archive_path: String,
    pub imported_games: usize,
    pub imported_files: usize,
    pub skipped_games: usize,
    pub skipped_files: usize,
    pub game_ids: Vec<String>,
}

const MAX_HTML_FILE_BYTES: u64 = 80 * 1024 * 1024;
const MAX_GAME_FOLDER_BYTES: u64 = 500 * 1024 * 1024;
const MAX_GAME_FOLDER_FILES: usize = 5_000;

#[derive(Default)]
struct ImportPlan {
    games: BTreeMap<String, GameArchivePlan>,
    categories: Option<Vec<u8>>,
    skipped_files: usize,
}

#[derive(Default)]
struct GameArchivePlan {
    files: Vec<GameArchiveFile>,
    total_bytes: u64,
}

struct GameArchiveFile {
    zip_index: usize,
    resource_path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameMetadata {
    entry: Option<String>,
}

pub fn import_games_archive<R: Runtime>(
    app: &AppHandle<R>,
    archive_path: String,
) -> Result<ImportGamesArchiveSummary, String> {
    let archive_path_buf = PathBuf::from(&archive_path);
    if !archive_path_buf.is_file() {
        return Err("Khong tim thay file zip can nhap.".to_string());
    }

    let archive_file = File::open(&archive_path_buf)
        .map_err(|err| format!("Khong mo duoc file zip can nhap: {err}"))?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|err| format!("File zip kho game khong hop le: {err}"))?;
    let plan = build_import_plan(&mut archive)?;

    if plan.games.is_empty() && plan.categories.is_none() {
        return Err("File zip khong co game hop le de nhap.".to_string());
    }

    let installed_ids = catalog::list_games(app)?
        .into_iter()
        .map(|game| game.id)
        .collect::<HashSet<_>>();
    let target_root = catalog::installed_games_dir(app)?;
    fs::create_dir_all(&target_root)
        .map_err(|err| format!("Khong tao duoc thu muc games trong app: {err}"))?;

    let staging_root = staging_dir(app)?;
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .map_err(|err| format!("Khong don duoc thu muc tam import: {err}"))?;
    }
    fs::create_dir_all(&staging_root)
        .map_err(|err| format!("Khong tao duoc thu muc tam import: {err}"))?;

    let import_result = import_planned_games(
        &mut archive,
        &plan,
        &installed_ids,
        &target_root,
        &staging_root,
    );
    let _ = fs::remove_dir_all(&staging_root);

    let (imported_files, skipped_games, mut skipped_files, imported_ids) = import_result?;
    skipped_files += plan.skipped_files;

    let imported_categories = if let Some(categories) = plan.categories.as_deref() {
        write_imported_categories(app, categories)?;
        true
    } else {
        false
    };

    if imported_ids.is_empty() && !imported_categories {
        return Err("Khong co game moi hop le nao trong file zip.".to_string());
    }

    catalog::rebuild_catalog_cache(app)?;

    Ok(ImportGamesArchiveSummary {
        archive_path,
        imported_games: imported_ids.len(),
        imported_files,
        skipped_games,
        skipped_files,
        game_ids: imported_ids,
    })
}

fn build_import_plan<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<ImportPlan, String> {
    let mut plan = ImportPlan::default();

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("Khong doc duoc entry zip: {err}"))?;
        let name = file.name().to_string();
        if file.is_dir() {
            continue;
        }
        let Some(parts) = valid_zip_entry_parts(&name)? else {
            plan.skipped_files += 1;
            continue;
        };

        if parts.len() == 1 && parts[0] == "categories.json" {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|err| format!("Khong doc duoc categories.json trong zip: {err}"))?;
            serde_json::from_slice::<Vec<Category>>(&bytes)
                .map_err(|err| format!("categories.json trong zip khong hop le: {err}"))?;
            plan.categories = Some(bytes);
            continue;
        }

        if parts.len() < 3 || parts[0] != "games" {
            plan.skipped_files += 1;
            continue;
        }

        let game_id = parts[1].to_string();
        validate_game_id(&game_id)?;
        let resource_path = parts[2..].join("/");
        validate_resource_path(&resource_path)?;
        if should_skip_resource(&resource_path) {
            plan.skipped_files += 1;
            continue;
        }

        let size = file.size();
        if is_html_resource(&resource_path) && size > MAX_HTML_FILE_BYTES {
            plan.skipped_files += 1;
            continue;
        }
        let game = plan.games.entry(game_id).or_default();
        game.total_bytes = game.total_bytes.saturating_add(size);
        if game.files.len() >= MAX_GAME_FOLDER_FILES || game.total_bytes > MAX_GAME_FOLDER_BYTES {
            plan.skipped_files += 1;
            continue;
        }
        game.files.push(GameArchiveFile {
            zip_index: index,
            resource_path,
        });
    }

    Ok(plan)
}

fn import_planned_games<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    plan: &ImportPlan,
    installed_ids: &HashSet<String>,
    target_root: &Path,
    staging_root: &Path,
) -> Result<(usize, usize, usize, Vec<String>), String> {
    let mut imported_files = 0;
    let mut skipped_games = 0;
    let mut skipped_files = 0;
    let mut imported_ids = Vec::new();

    for (game_id, game_plan) in &plan.games {
        if installed_ids.contains(game_id) || target_root.join(game_id).exists() {
            skipped_games += 1;
            skipped_files += game_plan.files.len();
            continue;
        }
        if game_plan.files.is_empty() {
            skipped_games += 1;
            continue;
        }

        let staged_game_dir = staging_root.join(game_id);
        fs::create_dir_all(&staged_game_dir)
            .map_err(|err| format!("Khong tao duoc thu muc tam cho {game_id}: {err}"))?;

        let mut copied_for_game = 0;
        for entry in &game_plan.files {
            let mut zip_file = archive
                .by_index(entry.zip_index)
                .map_err(|err| format!("Khong doc duoc file zip cho {game_id}: {err}"))?;
            let target_path = child_path(&staged_game_dir, &entry.resource_path)?;
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("Khong tao duoc thu muc import {game_id}: {err}"))?;
            }
            let mut target_file = File::create(&target_path)
                .map_err(|err| format!("Khong tao duoc file import {game_id}: {err}"))?;
            std::io::copy(&mut zip_file, &mut target_file)
                .map_err(|err| format!("Khong ghi duoc file import {game_id}: {err}"))?;
            copied_for_game += 1;
        }

        if !staged_game_has_entry(&staged_game_dir)? {
            skipped_games += 1;
            skipped_files += copied_for_game;
            let _ = fs::remove_dir_all(&staged_game_dir);
            continue;
        }

        let final_game_dir = target_root.join(game_id);
        fs::rename(&staged_game_dir, &final_game_dir)
            .map_err(|err| format!("Khong hoan tat import game {game_id}: {err}"))?;
        imported_files += copied_for_game;
        imported_ids.push(game_id.clone());
    }

    Ok((imported_files, skipped_games, skipped_files, imported_ids))
}

fn staged_game_has_entry(game_dir: &Path) -> Result<bool, String> {
    let metadata_path = game_dir.join("game.json");
    if metadata_path.is_file() {
        let metadata = fs::read_to_string(&metadata_path)
            .map_err(|err| format!("Khong doc duoc game.json trong zip: {err}"))?;
        if let Ok(metadata) = serde_json::from_str::<GameMetadata>(&metadata) {
            if let Some(entry) = metadata.entry.as_deref() {
                if validate_resource_path(entry).is_ok() && game_dir.join(entry).is_file() {
                    return Ok(true);
                }
            }
        }
    }

    Ok(game_dir.join("index.html").is_file())
}

fn write_imported_categories<R: Runtime>(app: &AppHandle<R>, bytes: &[u8]) -> Result<(), String> {
    let categories_path = app
        .path()
        .app_data_dir()
        .map(|dir| dir.join("categories.json"))
        .map_err(|err| format!("Khong xac dinh duoc file categories cua app: {err}"))?;
    if let Some(parent) = categories_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Khong tao duoc thu muc categories: {err}"))?;
    }
    fs::write(categories_path, bytes)
        .map_err(|err| format!("Khong ghi duoc categories import: {err}"))
}

fn staging_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(format!("import-staging-{timestamp}")))
        .map_err(|err| format!("Khong xac dinh duoc thu muc tam import: {err}"))
}

fn valid_zip_entry_parts(name: &str) -> Result<Option<Vec<&str>>, String> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') {
        return Err("Zip co duong dan file khong hop le.".to_string());
    }
    let parts = name
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(None);
    }
    if parts
        .iter()
        .any(|part| *part == "." || *part == ".." || part.contains('\0'))
    {
        return Err("Zip co duong dan traversal khong hop le.".to_string());
    }
    Ok(Some(parts))
}

fn validate_resource_path(resource_path: &str) -> Result<(), String> {
    if resource_path.is_empty()
        || resource_path.starts_with('/')
        || resource_path.contains('\\')
        || resource_path
            .split('/')
            .any(|part| part == "." || part == ".." || part.is_empty() || part.contains('\0'))
    {
        return Err("Duong dan tai nguyen game trong zip khong hop le.".to_string());
    }
    Ok(())
}

fn child_path(parent: &Path, resource_path: &str) -> Result<PathBuf, String> {
    validate_resource_path(resource_path)?;
    let mut path = parent.to_path_buf();
    for part in resource_path.split('/') {
        path.push(part);
    }
    if path.starts_with(parent) {
        Ok(path)
    } else {
        Err("Duong dan import khong hop le.".to_string())
    }
}

fn should_skip_resource(resource_path: &str) -> bool {
    resource_path.split('/').any(|part| part == "__MACOSX")
        || resource_path
            .rsplit('/')
            .next()
            .map(|name| name == ".DS_Store" || name == "Thumbs.db")
            .unwrap_or(false)
}

fn is_html_resource(resource_path: &str) -> bool {
    resource_path
        .rsplit_once('.')
        .map(|(_, extension)| matches!(extension.to_ascii_lowercase().as_str(), "html" | "htm"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{build_import_plan, staged_game_has_entry, valid_zip_entry_parts};
    use std::{
        fs,
        io::{Cursor, Write},
        time::{SystemTime, UNIX_EPOCH},
    };
    use zip::{write::SimpleFileOptions, ZipWriter};

    fn zip_with_files(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut cursor);
            for (name, bytes) in files {
                zip.start_file(*name, SimpleFileOptions::default()).unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        cursor.into_inner()
    }

    #[test]
    fn plans_valid_archive_games() {
        let bytes = zip_with_files(&[
            ("games/game-a/index.html", b"<html></html>"),
            ("games/game-a/images/a.png", b"png"),
            ("games/game-b/game.json", br#"{"entry":"play.html"}"#),
            ("games/game-b/play.html", b"<html></html>"),
        ]);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let plan = build_import_plan(&mut archive).unwrap();
        assert_eq!(plan.games.len(), 2);
        assert_eq!(plan.games.get("game-a").unwrap().files.len(), 2);
        assert_eq!(plan.games.get("game-b").unwrap().files.len(), 2);
    }

    #[test]
    fn rejects_zip_path_traversal() {
        assert!(valid_zip_entry_parts("games/game-a/../secret.html").is_err());
        assert!(valid_zip_entry_parts("/games/game-a/index.html").is_err());
        assert!(valid_zip_entry_parts("games\\game-a\\index.html").is_err());
    }

    #[test]
    fn rejects_game_without_entry_html() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("yeutre-import-test-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("game.json"), br#"{"entry":"missing.html"}"#).unwrap();

        assert!(!staged_game_has_entry(&dir).unwrap());
        fs::remove_dir_all(dir).unwrap();
    }
}
