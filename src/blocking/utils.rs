use sha1::Digest;
use std::fs;

pub(crate) fn try_download_file(
    url: &str,
    path: &std::path::Path,
    hash: &str,
    retries: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = url.replace(std::path::MAIN_SEPARATOR_STR, "/");
    let url = url.as_str();
    let response = reqwest::blocking::get(url)?;
    let data = response.bytes()?;
    fs::write(path, data)?;

    if hash.len() != 40 {
        return Ok(());
    }

    if format!("{:x}", sha1::Sha1::digest(&fs::read(path)?)) != hash {
        if retries > 0 {
            fs::remove_file(path)?;
            try_download_file(url, path, hash, retries - 1)?;
        } else {
            return Err("Hash mismatch".into());
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
