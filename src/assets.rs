use crate::utils::{try_download_file, LauncherError};
use crate::Launcher;
use sha1::Digest;
use std::error::Error;
use tokio::fs;

impl Launcher {
    /// Install assets for the current version
    pub async fn install_assets(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.version.profile.is_null() {
            return Err(Box::from(LauncherError(
                "Please install a version before installing assets".to_string(),
            )));
        }

        println!("Checking assets");

        let assets_dir = self.game_dir.join("assets");
        let indexes_dir = assets_dir.join("indexes");
        let objects_dir = assets_dir.join("objects");

        fs::create_dir_all(&indexes_dir).await?;
        fs::create_dir_all(&objects_dir).await?;

        let index_path = indexes_dir.join(&format!(
            "{}.json",
            self.version.profile["assets"].as_str().unwrap()
        ));

        if !index_path.exists() {
            println!("Downloading asset index");

            let index_url = self.version.profile["assetIndex"]["url"].as_str().unwrap();
            let index_data = reqwest::get(index_url).await?.text().await?;
            fs::write(&index_path, index_data).await?;
        }

        let index: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path).await?)?;

        let mut tasks = Vec::new();

        let mut readdir = fs::read_dir(&objects_dir).await?;
        while let Some(file) = readdir.next_entry().await? {
            let path = file.path();
            if path.is_file() {
                let hash = path.file_name().unwrap().to_str().unwrap().to_string();

                if !index["objects"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|object| object["hash"].as_str().unwrap() == &hash)
                    || format!("{:x}", sha1::Sha1::digest(&fs::read(&path).await?)) != hash
                {
                    println!("Removing outdated asset {}", hash);
                    fs::remove_file(&path).await?;
                }
            }
        }

        for (name, object) in index["objects"].as_object().unwrap() {
            let object = object.as_object().unwrap();
            let hash = object["hash"].as_str().unwrap().to_string();

            let object_path = objects_dir.join(&hash[..2]).join(&hash);

            if !object_path.exists() {
                println!("Downloading assets/{}", name);

                fs::create_dir_all(object_path.parent().unwrap()).await?;

                let object_url = format!(
                    "https://resources.download.minecraft.net/{}",
                    hash[..2].to_string() + "/" + &hash
                );
                let download_task = async move {
                    try_download_file(&object_url, &object_path, &hash, 3).await?;

                    // Legacy assets
                    if self.version.profile["assets"].as_str().unwrap() == "legacy"
                        || self.version.profile["assets"].as_str().unwrap() == "pre-1.6"
                    {
                        let resources_path = self.game_dir.join("resources").join(name);
                        fs::create_dir_all(resources_path.parent().unwrap()).await?;
                        fs::copy(&object_path, &resources_path).await?;
                    }

                    Ok::<_, Box<dyn Error + Send + Sync>>(())
                };
                tasks.push(download_task);
            }
        }

        for task in tasks {
            task.await?;
        }

        Ok(())
    }
}
