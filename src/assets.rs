use crate::utils::{try_download_file, LauncherError};
use crate::Launcher;
use sha1::Digest;
use std::error::Error;
use tokio::fs;

impl Launcher {
    /// Install assets for the current version
    pub async fn install_assets(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.version.profile.is_null() {
            return Err(Box::from(LauncherError(
                "Please install a version before installing assets".to_string(),
            )));
        }

        self.emit_progress("checking_assets", "", 0, 0);

        let assets_dir = self.game_dir.join("assets");
        let indexes_dir = assets_dir.join("indexes");
        let objects_dir = assets_dir.join("objects");

        fs::create_dir_all(&indexes_dir).await?;
        fs::create_dir_all(&objects_dir).await?;

        self.fix_log4j_vulnerability().await?;

        let index_path = indexes_dir.join(&format!(
            "{}.json",
            self.version.profile["assets"].as_str().unwrap()
        ));

        if !index_path.exists() {
            let index_url = self.version.profile["assetIndex"]["url"].as_str().unwrap();
            let index_data = reqwest::get(index_url).await?.text().await?;
            fs::write(&index_path, index_data).await?;
        }

        let index: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&index_path).await?)?;

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
                    fs::remove_file(&path).await?;
                }
            }
        }

        let mut total: u64 = 0;
        let mut current: u64 = 0;
        let mut objects_to_download = vec![];

        for (name, object) in index["objects"].as_object().unwrap() {
            let object = object.as_object().unwrap();
            let hash = object["hash"].as_str().unwrap().to_string();

            let object_path = objects_dir.join(&hash[..2]).join(&hash);

            if !object_path.exists() {
                total += object["size"].as_u64().unwrap();
                objects_to_download.push({
                    let mut object = object.clone();
                    object.insert(
                        "name".to_string(),
                        serde_json::Value::String(name.to_string()),
                    );
                    object
                });
            }
        }

        if !objects_to_download.is_empty() {
            self.emit_progress("downloading_assets", "", total, 0);
        }

        for object in objects_to_download {
            let name = object["name"].as_str().unwrap();
            let hash = object["hash"].as_str().unwrap().to_string();
            let object_path = objects_dir.join(&hash[..2]).join(&hash);

            fs::create_dir_all(object_path.parent().unwrap()).await?;

            let object_url = format!(
                "https://resources.download.minecraft.net/{}",
                hash[..2].to_string() + "/" + &hash
            );

            try_download_file(&object_url, &object_path, &hash, 3).await?;

            current += object["size"].as_u64().unwrap();
            self.emit_progress("downloading_assets", name, total, current);

            // Legacy assets
            if self.version.profile["assets"].as_str().unwrap() == "legacy"
                || self.version.profile["assets"].as_str().unwrap() == "pre-1.6"
            {
                let resources_path = self
                    .game_dir
                    .join("resources")
                    .join(object["name"].as_str().unwrap());
                fs::create_dir_all(resources_path.parent().unwrap()).await?;
                fs::copy(&object_path, &resources_path).await?;
            }
        }

        Ok(())
    }

    async fn fix_log4j_vulnerability(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Fix log4j vulnerability
        if self.version.profile["logging"].is_object()
            && self.version.profile["logging"]["client"].is_object()
        {
            if (self.version.id.split('.').collect::<Vec<&str>>()[1] == "18"
                && self.version.id.split('.').collect::<Vec<&str>>().len() == 3)
                || self.version.id.split('.').collect::<Vec<&str>>()[1]
                    .parse::<u32>()
                    .unwrap()
                    > 18
            {
                return Ok(());
            }

            let log4j_path = self.game_dir.join("assets").join("log_configs").join(
                self.version.profile["logging"]["client"]["file"]["id"]
                    .as_str()
                    .unwrap(),
            );

            if !log4j_path.exists() {
                let log4j_url = self.version.profile["logging"]["client"]["file"]["url"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let log4j = reqwest::get(&log4j_url).await?.bytes().await?;
                fs::create_dir_all(log4j_path.parent().unwrap()).await?;
                fs::write(&log4j_path, log4j).await?;
            }

            let log4j_arg = self.version.profile["logging"]["client"]["argument"]
                .as_str()
                .unwrap()
                .replace("${path}", log4j_path.to_str().unwrap());
            self.args.push(log4j_arg);

            if self.version.id.split('.').collect::<Vec<&str>>()[1] == "18"
                && self.version.id.split('.').collect::<Vec<&str>>().len() == 2
                || self.version.id.split('.').collect::<Vec<&str>>()[1] == "17"
            {
                self.args
                    .push("-Dlog4j2.formatMsgNoLookups=true".to_string());
            }
        }

        Ok(())
    }
}
