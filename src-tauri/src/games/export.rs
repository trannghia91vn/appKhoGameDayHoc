use crate::games::catalog;
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use zip::{write::SimpleFileOptions, ZipWriter};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportGamesArchiveSummary {
    pub archive_path: String,
    pub exported_games: usize,
    pub exported_files: usize,
    pub archive_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportGamesArchiveProgress {
    exported_files: usize,
    current_path: String,
}

pub fn export_games_archive<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<ExportGamesArchiveSummary, String> {
    let games = catalog::list_games(app)?;
    if games.is_empty() {
        return Err("Chua co game nao trong kho de xuat.".to_string());
    }

    let games_dir = catalog::installed_games_dir(app)?;
    if !games_dir.is_dir() {
        return Err("Khong tim thay thu muc games da cai de xuat.".to_string());
    }

    let output_dir = app
        .path()
        .download_dir()
        .map_err(|err| format!("Khong xac dinh duoc thu muc Downloads: {err}"))?;
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("Khong tao duoc thu muc xuat zip: {err}"))?;

    let archive_path = unique_archive_path(&output_dir);
    let archive_file = File::create(&archive_path)
        .map_err(|err| format!("Khong tao duoc file zip xuat kho game: {err}"))?;
    let mut zip = ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default();
    let mut exported_files = 0;

    emit_export_progress(app, exported_files, "Bắt đầu xuất kho game");
    zip.add_directory("games/", options)
        .map_err(|err| format!("Khong tao duoc thu muc games trong zip: {err}"))?;
    add_directory_to_zip(
        app,
        &mut zip,
        &games_dir,
        &games_dir,
        "games",
        options,
        &mut exported_files,
    )?;

    let categories_path = app
        .path()
        .app_data_dir()
        .map(|dir| dir.join("categories.json"))
        .map_err(|err| format!("Khong xac dinh duoc file categories cua app: {err}"))?;
    if categories_path.is_file() {
        add_file_to_zip(&mut zip, &categories_path, "categories.json", options)?;
        exported_files += 1;
        emit_export_progress(app, exported_files, "categories.json");
    }

    zip.finish()
        .map_err(|err| format!("Khong hoan tat file zip kho game: {err}"))?;

    let archive_bytes = fs::metadata(&archive_path)
        .map_err(|err| format!("Khong doc duoc thong tin file zip da xuat: {err}"))?
        .len();

    emit_export_progress(app, exported_files, "Hoàn tất xuất kho game");

    Ok(ExportGamesArchiveSummary {
        archive_path: archive_path.to_string_lossy().to_string(),
        exported_games: games.len(),
        exported_files,
        archive_bytes,
    })
}

fn emit_export_progress<R: Runtime>(app: &AppHandle<R>, exported_files: usize, current_path: &str) {
    let _ = app.emit(
        "game-export-progress",
        ExportGamesArchiveProgress {
            exported_files,
            current_path: current_path.to_string(),
        },
    );
}

fn unique_archive_path(output_dir: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    output_dir.join(format!("yeutre-game-kho-{timestamp}.zip"))
}

fn add_directory_to_zip<R: Runtime, W: Write + std::io::Seek>(
    app: &AppHandle<R>,
    zip: &mut ZipWriter<W>,
    root_dir: &Path,
    current_dir: &Path,
    archive_root: &str,
    options: SimpleFileOptions,
    exported_files: &mut usize,
) -> Result<(), String> {
    let mut entries = fs::read_dir(current_dir)
        .map_err(|err| format!("Khong doc duoc thu muc can xuat {:?}: {err}", current_dir))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Khong doc duoc file trong thu muc xuat: {err}"))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("Khong doc duoc loai file khi xuat zip: {err}"))?;

        let relative_path = path
            .strip_prefix(root_dir)
            .map_err(|err| format!("Duong dan file xuat zip khong hop le: {err}"))?;
        let archive_name = format!(
            "{archive_root}/{}",
            relative_path
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        );

        if file_type.is_dir() {
            zip.add_directory(format!("{archive_name}/"), options)
                .map_err(|err| format!("Khong them duoc thu muc vao zip {archive_name}: {err}"))?;
            add_directory_to_zip(app, zip, root_dir, &path, archive_root, options, exported_files)?;
        } else if file_type.is_file() {
            add_file_to_zip(zip, &path, &archive_name, options)?;
            *exported_files += 1;
            if *exported_files == 1 || *exported_files % 25 == 0 {
                emit_export_progress(app, *exported_files, &archive_name);
            }
        }
    }

    Ok(())
}

fn add_file_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    path: &Path,
    archive_name: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    zip.start_file(archive_name, options)
        .map_err(|err| format!("Khong them duoc file vao zip {archive_name}: {err}"))?;

    let mut file = File::open(path)
        .map_err(|err| format!("Khong doc duoc file de xuat zip {:?}: {err}", path))?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("Khong doc duoc noi dung file {:?}: {err}", path))?;
        if read == 0 {
            break;
        }
        zip.write_all(&buffer[..read])
            .map_err(|err| format!("Khong ghi duoc file vao zip {archive_name}: {err}"))?;
    }

    Ok(())
}
