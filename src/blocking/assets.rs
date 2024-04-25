use crate::blocking::utils::try_download_file;
use crate::blocking::Launcher;
use sha1::Digest;
use std::fs;

impl Launcher {
    /// Install assets for the current version
    pub fn install_assets(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.version.profile.is_null() {
            return Err("Please install a version before installing assets".into());
        }

        println!("Checking assets");

        let assets_dir = self.game_dir.join("assets");
        let indexes_dir = assets_dir.join("indexes");
        let objects_dir = assets_dir.join("objects");

        fs::create_dir_all(&indexes_dir)?;
        fs::create_dir_all(&objects_dir)?;

        let index_path = indexes_dir.join(&format!(
            "{}.json",
            self.version.profile["assets"].as_str().unwrap()
        ));

        if !index_path.exists() {
            println!("Downloading asset index");

            let index_url = self.version.profile["assetIndex"]["url"].as_str().unwrap();
            let index_data = reqwest::blocking::get(index_url)?.text()?;
            fs::write(&index_path, index_data)?;
        }

        let index: serde_json::Value = serde_json::from_str(&fs::read_to_string(&index_path)?)?;

        for file in fs::read_dir(&objects_dir)? {
            let file = file?;
            let path = file.path();
            if path.is_file() {
                let hash = path.file_name().unwrap().to_str().unwrap();

                if !index["objects"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|object| object["hash"].as_str().unwrap() == hash)
                    || format!("{:x}", sha1::Sha1::digest(&fs::read(&path)?)) != hash
                {
                    println!("Removing outdated asset {}", hash);
                    fs::remove_file(&path)?;
                }
            }
        }

        for (name, object) in index["objects"].as_object().unwrap() {
            let object = object.as_object().unwrap();
            let hash = object["hash"].as_str().unwrap();

            let object_path = objects_dir.join(&hash[..2]).join(hash);

            if !object_path.exists() {
                println!("Downloading assets/{}", name);

                fs::create_dir_all(object_path.parent().unwrap())?;

                let object_url = format!(
                    "https://resources.download.minecraft.net/{}",
                    hash[..2].to_string() + "/" + hash
                );
                try_download_file(&object_url, &object_path, hash, 3)?;

                // Legacy assets
                if self.version.profile["assets"].as_str().unwrap() == "legacy"
                    || self.version.profile["assets"].as_str().unwrap() == "pre-1.6"
                {
                    let resources_path = self.game_dir.join("resources").join(name);
                    fs::create_dir_all(resources_path.parent().unwrap())?;
                    fs::copy(&object_path, &resources_path)?;
                }
            }
        }

        Ok(())
    }
}
