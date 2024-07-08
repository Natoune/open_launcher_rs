use async_recursion::async_recursion;
use serde_json::Value;
use sha1::Digest;
use std::error::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[derive(Debug)]
pub struct LauncherError(pub String);

impl std::fmt::Display for LauncherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for LauncherError {}

impl From<LauncherError> for Box<dyn Error + Send> {
    fn from(error: LauncherError) -> Self {
        Box::new(error) as Box<dyn Error + Send>
    }
}

#[async_recursion]
pub(crate) async fn try_download_file(
    url: &str,
    path: &std::path::Path,
    hash: &str,
    retries: u32,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let url = url.replace(std::path::MAIN_SEPARATOR_STR, "/");
    let url = url.as_str();

    let response = reqwest::get(url).await?;
    let data = response.bytes().await?;

    let mut file = fs::File::create(path).await?;
    file.write_all(&data).await?;
    file.sync_all().await?;
    file.flush().await?;

    if hash.len() != 40 {
        return Ok(());
    }

    let downloaded_hash = format!("{:x}", sha1::Sha1::digest(&fs::read(path).await?));

    if downloaded_hash != hash {
        if retries > 0 {
            fs::remove_file(path).await?;
            try_download_file(url, path, hash, retries - 1).await?;
        } else {
            return Err(Box::from(LauncherError(format!(
                "Failed to download file: {}",
                path.display()
            ))));
        }
    }

    Ok(())
}

pub(crate) fn get_os() -> String {
    match std::env::consts::OS {
        "windows" => "windows".to_string(),
        "macos" => "osx".to_string(),
        "linux" => "linux".to_string(),
        _ => std::env::consts::OS.to_string(),
    }
}

pub(crate) async fn extract_file(
    zip_path: &std::path::Path,
    file_name: &str,
    extract_path: &std::path::Path,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if extract_path.exists() {
        return Ok(());
    }

    let archive = async_zip::tokio::read::fs::ZipFileReader::new(zip_path).await?;

    for i in 0..archive.file().entries().len() {
        if archive
            .file()
            .entries()
            .get(i)
            .unwrap()
            .filename()
            .as_str()?
            == file_name
        {
            if archive.file().entries().get(i).unwrap().dir()? {
                fs::create_dir_all(extract_path).await?;
            } else {
                let mut reader = archive.reader_without_entry(i).await?;
                if !extract_path.parent().unwrap().exists() {
                    fs::create_dir_all(extract_path.parent().unwrap()).await?;
                }

                let writer = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&extract_path)
                    .await?;

                futures_lite::io::copy(&mut reader, &mut writer.compat_write()).await?;

                return Ok(());
            }
        }
    }

    Ok(())
}

pub(crate) async fn extract_all(
    zip_path: &std::path::Path,
    extract_path: &std::path::Path,
) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let archive = async_zip::tokio::read::fs::ZipFileReader::new(zip_path).await?;
    let mut extracted = vec![];

    for i in 0..archive.file().entries().len() {
        let entry = archive.file().entries().get(i).unwrap();
        let path = extract_path.join(entry.filename().as_str()?);

        if path.exists() {
            extracted.push(serde_json::json!({
                "path": path,
                "hash": format!("{:x}", sha1::Sha1::digest(&fs::read(&path).await?)),
            }));
            continue;
        }

        if entry.filename().as_str()?.ends_with(".git")
            || entry.filename().as_str()?.ends_with(".sha1")
            || entry.filename().as_str()?.starts_with("META-INF")
        {
            continue;
        }

        if entry.dir()? {
            fs::create_dir_all(path).await?;
        } else {
            let mut reader = archive.reader_without_entry(i).await?;
            if !path.parent().unwrap().exists() {
                fs::create_dir_all(path.parent().unwrap()).await?;
            }

            let writer = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await?;

            futures_lite::io::copy(&mut reader, &mut writer.compat_write()).await?;

            extracted.push(serde_json::json!({
                "path": path,
                "hash": format!("{:x}", sha1::Sha1::digest(&fs::read(&path).await?)),
            }));
        }
    }

    Ok(extracted)
}
