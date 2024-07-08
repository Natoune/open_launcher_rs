use crate::utils::get_os;
use crate::utils::{extract_all, try_download_file};
use crate::Launcher;
use crate::{events, forge};
use serde_json::Value;
use sha1::Digest;
use std::error::Error;
use std::path::Path;
use tokio::fs;
use tokio::sync::broadcast;

pub(crate) fn get_lib_path(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    let group = parts[0].replace(".", std::path::MAIN_SEPARATOR_STR);
    let artifact = parts[1].to_string();
    let version = match parts[2].find('@') {
        Some(index) => parts[2][..index].to_string(),
        None => parts[2].to_string(),
    };
    let classifier = match parts.get(3) {
        Some(classifier) => match classifier.find('@') {
            Some(index) => format!("-{}", classifier[..index].to_string()),
            None => format!("-{}", classifier.to_string()),
        },
        None => "".to_string(),
    };
    let extension = match name.find('@') {
        Some(index) => name[index + 1..].to_string(),
        None => "jar".to_string(),
    };

    group
        + std::path::MAIN_SEPARATOR_STR
        + &artifact
        + std::path::MAIN_SEPARATOR_STR
        + &version
        + std::path::MAIN_SEPARATOR_STR
        + &format!("{}-{}{}.{}", artifact, version, classifier, extension)
}

pub(crate) fn allowed_rule(library: &Value) -> bool {
    let mut allowed = false;

    if let Some(rules) = library.get("rules") {
        for rule in rules.as_array().unwrap() {
            let rule = rule.as_object().unwrap();
            let action = rule["action"].as_str().unwrap();
            let os = rule.get("os");

            if action == "allow" {
                if os.is_none() {
                    allowed = true;
                } else {
                    let os = os.unwrap().as_object().unwrap();
                    let name = os.get("name").unwrap().as_str().unwrap();
                    if name == get_os() {
                        allowed = true;
                    }
                }
            } else if action == "disallow" {
                if os.is_none() {
                    allowed = false;
                } else {
                    let os = os.unwrap().as_object().unwrap();
                    let name = os.get("name").unwrap().as_str().unwrap();
                    if name == get_os() {
                        allowed = false;
                    }
                }
            }
        }

        allowed
    } else {
        true
    }
}

pub(crate) async fn sort_libs(
    libs: &Vec<Value>,
    libraries_dir: &Path,
    base_url: &str,
) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let mut libraries_vec = vec![];

    for library in libs {
        let name = library["name"].as_str().unwrap();
        let base_url = match library {
            Value::Object(library) => match library.get("url") {
                Some(url) => url.as_str().unwrap(),
                None => base_url,
            },
            _ => base_url,
        };
        let url = format!("{}{}", base_url, get_lib_path(name));
        let url = match library {
            Value::Object(library) => match library.get("downloads") {
                Some(downloads) => match downloads.get("artifact") {
                    Some(artifact) => match artifact.get("url") {
                        Some(url) => url.as_str().unwrap().to_string(),
                        None => url,
                    },
                    None => url,
                },
                None => url,
            },
            Value::String(_) => url,
            _ => url,
        };
        let hash = match library {
            Value::Object(library) => match library.get("downloads") {
                Some(downloads) => match downloads.get("artifact") {
                    Some(artifact) => match artifact.get("sha1") {
                        Some(sha1) => sha1.as_str().unwrap(),
                        None => "",
                    },
                    None => "",
                },
                None => "",
            },
            Value::String(_) => "",
            _ => "",
        };

        let path = libraries_dir.join(get_lib_path(name));

        if !path.exists() && allowed_rule(library) {
            libraries_vec.push(serde_json::json!({
                "name": name,
                "url": url,
                "hash": hash,
                "path": path.to_str().unwrap(),
            }));
        }
    }

    Ok(libraries_vec)
}

pub(crate) async fn download_libs(
    libs: &Vec<Value>,
    progress: &mut events::Progress,
    progress_sender: broadcast::Sender<events::Progress>,
) -> Result<events::Progress, Box<dyn Error + Send + Sync>> {
    for library in libs {
        let name = library["name"].as_str().unwrap();
        let url = library["url"].as_str().unwrap();
        let hash = library["hash"].as_str().unwrap();
        let path = Path::new(library["path"].as_str().unwrap());

        fs::create_dir_all(path.parent().unwrap()).await?;
        try_download_file(url, path, hash, 3).await?;

        *progress = events::Progress {
            task: "downloading_libraries".to_string(),
            file: name.to_string(),
            total: progress.total,
            current: progress.current + 1,
        };
        let _ = progress_sender.send(progress.clone());
    }

    Ok(progress.clone())
}

pub(crate) async fn sort_natives(
    natives: &Vec<Value>,
    natives_dir: &std::path::Path,
) -> Vec<Value> {
    let mut natives_vec = vec![];

    for library in natives {
        let library = library.as_object().unwrap();
        let name = library["name"].as_str().unwrap();

        if library.get("downloads").is_none() {
            continue;
        }

        let classifiers = library["downloads"].get("classifiers");

        if classifiers.is_none() {
            continue;
        }

        let classifiers = classifiers.unwrap().as_object().unwrap();
        let natives = classifiers.get(&("natives-".to_string() + get_os().as_str()));

        if natives.is_none() {
            continue;
        }

        let natives = natives.unwrap().as_object().unwrap();
        let hash = natives["sha1"].as_str().unwrap();
        let parts: Vec<&str> = name.split(':').collect();
        let artifact = parts[1];
        let version = parts[2];
        let path = natives_dir.join(
            &format!(
                "{}-{}-natives-{}.jar",
                artifact,
                version,
                get_os().replace("windows", "win")
            )
            .replace("linux", "nix"),
        );

        let natives_json = natives_dir.join("natives.json");
        if natives_json.exists() {
            let mut ok = true;

            let natives_json_content = fs::read_to_string(natives_json).await.unwrap();
            let natives_json_content: serde_json::Value =
                serde_json::from_str(&natives_json_content).expect("Failed to parse natives.json");

            if natives_json_content[name].is_array() {
                let extracted = natives_json_content[name].as_array().unwrap();
                for native in extracted {
                    let native: &serde_json::Map<String, Value> = native.as_object().unwrap();
                    if native["hash"].as_str().unwrap()
                        != format!(
                            "{:x}",
                            sha1::Sha1::digest(
                                &fs::read(native["path"].as_str().unwrap_or_else(|| ""))
                                    .await
                                    .unwrap()
                            )
                        )
                    {
                        ok = false;
                        fs::remove_file(Path::new(native["path"].as_str().unwrap()))
                            .await
                            .unwrap();
                    }
                }

                if ok {
                    continue;
                }
            }
        }

        natives_vec.push(serde_json::json!({
            "name": name,
            "url": natives["url"].as_str().unwrap(),
            "hash": hash,
            "path": path.to_str().unwrap(),
        }));
    }

    natives_vec
}

pub(crate) async fn extract_natives(
    natives: &Vec<Value>,
    natives_dir: &std::path::Path,
    progress: &mut events::Progress,
    progress_sender: broadcast::Sender<events::Progress>,
) -> Result<events::Progress, Box<dyn Error + Send + Sync>> {
    if natives.is_empty() {
        return Ok(progress.clone());
    }

    let natives_json = natives_dir.join("natives.json");
    let mut natives_json_content = if natives_json.clone().exists() {
        let natives_json_content = fs::read_to_string(&natives_json).await.unwrap();
        serde_json::from_str(&natives_json_content).unwrap_or_else(|_| serde_json::Map::new())
    } else {
        serde_json::Map::new()
    };

    for library in natives {
        let name = library["name"].as_str().unwrap();
        let url = library["url"].as_str().unwrap();
        let hash = library["hash"].as_str().unwrap();
        let path = Path::new(library["path"].as_str().unwrap());

        fs::create_dir_all(path.parent().unwrap()).await?;
        try_download_file(url, path, hash, 3).await?;

        // Extract natives jar
        let extracted = extract_all(&path, &natives_dir).await?;

        // Remove natives jar
        fs::remove_file(path).await?;

        // Add native to natives.json
        natives_json_content.insert(name.to_string(), serde_json::Value::Array(extracted));

        *progress = events::Progress {
            task: "extracting_natives".to_string(),
            file: name.to_string(),
            total: progress.total,
            current: progress.current + 1,
        };
        let _ = progress_sender.send(progress.clone());
    }

    fs::create_dir_all(natives_dir).await?;
    fs::write(
        natives_json,
        serde_json::Value::Object(natives_json_content).to_string(),
    )
    .await
    .unwrap();

    Ok(progress.clone())
}

pub(crate) fn get_libraries_classpath(
    game_dir: &std::path::PathBuf,
    libraries: &Vec<Value>,
) -> Vec<String> {
    let mut classpath = Vec::new();

    for library in libraries {
        let name = match library {
            serde_json::Value::Object(library) => library["name"].as_str().unwrap(),
            serde_json::Value::String(library) => library.as_str(),
            _ => "",
        };

        let path = game_dir.join("libraries").join(get_lib_path(name));
        if path.exists()
            && !classpath.contains(&path.to_str().unwrap().to_string())
            && allowed_rule(library)
        {
            classpath.push(path.to_str().unwrap().to_string());
        }
    }

    classpath
}

impl Launcher {
    /// Install libraries for the current version
    pub async fn install_libraries(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.version.profile.is_null() {
            return Err("Please install a version before installing libraries".into());
        }

        self.emit_progress("checking_libraries", "", 0, 0);

        let libraries_dir = self.game_dir.join("libraries");
        let natives_dir = self
            .game_dir
            .join("versions")
            .join(format!("{}-natives", &self.version.id));

        /* LIBRARIES */
        // Get libraries
        let vanilla_libs = sort_libs(
            &self.version.profile["libraries"].as_array().unwrap(),
            &libraries_dir,
            "https://libraries.minecraft.net/",
        )
        .await
        .unwrap();
        let modded_libs = if self.version.modded_profile.is_object() {
            sort_libs(
                &self.version.modded_profile["libraries"].as_array().unwrap(),
                &libraries_dir,
                if self.version.forge.enabled {
                    "https://maven.creeperhost.net/"
                } else if self.version.neoforge.enabled {
                    "https://maven.neoforged.net/releases/"
                } else if self.version.fabric.enabled {
                    "https://maven.fabricmc.net/"
                } else if self.version.quilt.enabled {
                    "https://maven.quiltmc.org/repository/release/"
                } else {
                    "https://libraries.minecraft.net/"
                },
            )
            .await
            .unwrap()
        } else {
            vec![]
        };
        let post_processing_libs = if (self.version.forge.enabled && !self.version.forge.legacy)
            || self.version.neoforge.enabled
        {
            sort_libs(
                &self.version.forge.install_profile["libraries"]
                    .as_array()
                    .unwrap(),
                &libraries_dir,
                if self.version.forge.enabled {
                    "https://maven.creeperhost.net/"
                } else {
                    "https://maven.neoforged.net/releases/"
                },
            )
            .await
            .unwrap()
        } else {
            vec![]
        };

        let mut libs = vanilla_libs.clone();
        for lib in modded_libs {
            if !libs.contains(&lib) {
                libs.push(lib);
            }
        }
        for lib in post_processing_libs {
            if !libs.contains(&lib) {
                libs.push(lib);
            }
        }

        // Downloading libraries
        self.emit_progress("downloading_libraries", "", libs.len() as u64, 0);

        let mut error = None;
        self.progress = download_libs(
            &libs,
            &mut self.progress.clone(),
            self.progress_sender.clone(),
        )
        .await
        .unwrap_or_else(|e| {
            error = Some(e);
            self.progress.clone()
        });

        if let Some(e) = error {
            return Err(e);
        }

        /* FORGE POST PROCESSING */
        if (self.version.forge.enabled && !self.version.forge.legacy)
            || self.version.neoforge.enabled
        {
            let mut error = None;

            forge::post_process(
                &self.game_dir,
                &self.java_executable,
                &self.version.forge.install_profile,
                self.progress_sender.clone(),
            )
            .await
            .unwrap_or_else(|e| {
                error = Some(e);
            });

            if let Some(e) = error {
                return Err(e);
            }
        }

        /* NATIVES */
        // Get natives
        let vanilla_natives = sort_natives(
            &self.version.profile["libraries"].as_array().unwrap(),
            &natives_dir,
        )
        .await;
        let modded_natives = if self.version.modded_profile.is_object() {
            sort_natives(
                &self.version.modded_profile["libraries"].as_array().unwrap(),
                &natives_dir,
            )
            .await
        } else {
            vec![]
        };

        let mut natives = vanilla_natives.clone();
        for native in modded_natives {
            if !natives.contains(&native) {
                natives.push(native);
            }
        }

        self.emit_progress("checking_natives", "", natives.len() as u64, 0);

        // Download natives
        let mut error = None;
        self.progress = extract_natives(
            &natives,
            &natives_dir,
            &mut self.progress.clone(),
            self.progress_sender.clone(),
        )
        .await
        .unwrap_or_else(|e| {
            error = Some(e);
            self.progress.clone()
        });

        if let Some(e) = error {
            return Err(e);
        }

        Ok(())
    }

    pub(crate) fn get_classpath(&self) -> Vec<String> {
        let mut classpath = get_libraries_classpath(
            &self.game_dir,
            &self.version.profile["libraries"].as_array().unwrap(),
        );

        if self.version.modded_profile.is_object()
            && self.version.modded_profile["libraries"].is_array()
        {
            let modded_classpath = get_libraries_classpath(
                &self.game_dir,
                &self.version.modded_profile["libraries"].as_array().unwrap(),
            );
            for path in modded_classpath {
                if !classpath.contains(&path) {
                    classpath.push(path);
                }
            }
        }

        classpath
    }
}
