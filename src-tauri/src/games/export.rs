use crate::games::catalog;
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
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
    let temp_path = temp_archive_path(&archive_path);
    let write_result = write_archive_to_temp(app, &games_dir, &temp_path);

    match write_result {
        Ok((exported_files, archive_bytes)) => {
            fs::rename(&temp_path, &archive_path).map_err(|err| {
                format!("Khong doi ten file zip tam thanh file xuat chinh: {err}")
            })?;
            emit_export_progress(app, exported_files, "Hoàn tất xuất kho game");

            Ok(ExportGamesArchiveSummary {
                archive_path: archive_path.to_string_lossy().to_string(),
                exported_games: games.len(),
                exported_files,
                archive_bytes,
            })
        }
        Err(message) => {
            let _ = fs::remove_file(&temp_path);
            Err(message)
        }
    }
}

fn export_archive_folder(downloads_dir: &Path, archive_path: &Path) -> Result<PathBuf, String> {
    let archive = fs::canonicalize(archive_path)
        .map_err(|err| format!("Không tìm thấy file ZIP đã xuất: {err}"))?;
    let downloads = fs::canonicalize(downloads_dir)
        .map_err(|err| format!("Không tìm thấy thư mục Downloads: {err}"))?;
    let name = archive
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if !archive.is_file()
        || archive.parent() != Some(downloads.as_path())
        || !name.starts_with("yeutre-game-kho-")
        || !name.ends_with(".zip")
    {
        return Err("File ZIP không thuộc kho game đã xuất trong Downloads.".to_string());
    }
    Ok(downloads)
}

pub fn open_export_archive_folder<R: Runtime>(
    app: &AppHandle<R>,
    archive_path: &str,
) -> Result<(), String> {
    let downloads = app
        .path()
        .download_dir()
        .map_err(|err| format!("Không xác định được thư mục Downloads: {err}"))?;
    let folder = export_archive_folder(&downloads, Path::new(archive_path))?;

    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("explorer.exe");
        command.arg(&folder).creation_flags(0x08000000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&folder);
        command
    };
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&folder);
        command
    };

    command
        .spawn()
        .map_err(|err| format!("Không mở được thư mục chứa file ZIP: {err}"))?;
    Ok(())
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

fn write_archive_to_temp<R: Runtime>(
    app: &AppHandle<R>,
    games_dir: &Path,
    temp_path: &Path,
) -> Result<(usize, u64), String> {
    if temp_path.exists() {
        fs::remove_file(temp_path)
            .map_err(|err| format!("Khong xoa duoc file zip tam cu: {err}"))?;
    }

    let archive_file = File::create(temp_path)
        .map_err(|err| format!("Khong tao duoc file zip tam de xuat kho game: {err}"))?;
    let mut zip = ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default();
    let mut exported_files = 0;

    emit_export_progress(app, exported_files, "Bắt đầu xuất kho game");
    zip.add_directory("games/", options)
        .map_err(|err| format!("Khong tao duoc thu muc games trong zip: {err}"))?;
    add_directory_to_zip(
        app,
        &mut zip,
        games_dir,
        games_dir,
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

    let archive_bytes = fs::metadata(temp_path)
        .map_err(|err| format!("Khong doc duoc thong tin file zip da xuat: {err}"))?
        .len();

    Ok((exported_files, archive_bytes))
}

fn unique_archive_path(output_dir: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    output_dir.join(format!("yeutre-game-kho-{timestamp}.zip"))
}

fn temp_archive_path(archive_path: &Path) -> PathBuf {
    let file_name = archive_path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "yeutre-game-kho.zip".into());
    archive_path.with_file_name(format!(".{file_name}.tmp"))
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
            add_directory_to_zip(
                app,
                zip,
                root_dir,
                &path,
                archive_root,
                options,
                exported_files,
            )?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_opens_exported_zip_in_downloads() {
        let root = std::env::temp_dir().join(format!(
            "yeutre-export-folder-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let downloads = root.join("Downloads");
        let other = root.join("Other");
        fs::create_dir_all(&downloads).unwrap();
        fs::create_dir_all(&other).unwrap();
        let valid = downloads.join("yeutre-game-kho-123.zip");
        let wrong_name = downloads.join("another.zip");
        let wrong_folder = other.join("yeutre-game-kho-123.zip");
        fs::write(&valid, []).unwrap();
        fs::write(&wrong_name, []).unwrap();
        fs::write(&wrong_folder, []).unwrap();

        assert_eq!(
            export_archive_folder(&downloads, &valid).unwrap(),
            fs::canonicalize(&downloads).unwrap()
        );
        assert!(export_archive_folder(&downloads, &wrong_name).is_err());
        assert!(export_archive_folder(&downloads, &wrong_folder).is_err());
        assert!(export_archive_folder(&downloads, &downloads.join("missing.zip")).is_err());

        fs::remove_dir_all(root).unwrap();
    }
}
