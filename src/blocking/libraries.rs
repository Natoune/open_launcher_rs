use serde_json::Value;

use crate::blocking::forge;
use crate::blocking::utils::get_os;
use crate::blocking::utils::try_download_file;
use crate::blocking::Launcher;
use std::fs;
use std::io::Read;

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

pub(crate) fn process_libs(
    libs: &Vec<Value>,
    libraries_dir: &std::path::Path,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for library in libs {
        let name = match library {
            serde_json::Value::Object(library) => library["name"].as_str().unwrap(),
            serde_json::Value::String(library) => library.as_str(),
            _ => "",
        };
        let base_url = match library {
            serde_json::Value::Object(library) => match library.get("url") {
                Some(url) => url.as_str().unwrap(),
                None => base_url,
            },
            _ => base_url,
        };
        let hash = match library {
            serde_json::Value::Object(library) => match library.get("downloads") {
                Some(downloads) => match downloads.get("artifact") {
                    Some(artifact) => match artifact.get("sha1") {
                        Some(sha1) => sha1.as_str().unwrap(),
                        None => "",
                    },
                    None => "",
                },
                None => "",
            },
            serde_json::Value::String(_) => "",
            _ => "",
        };

        let path = libraries_dir.join(get_lib_path(name));
        if !path.exists() {
            if let Some(rules) = library.get("rules") {
                let mut allowed = false;
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

                if !allowed {
                    continue;
                }
            }

            fs::create_dir_all(path.parent().unwrap())?;

            let url = format!("{}{}", base_url, get_lib_path(name));
            try_download_file(&url, &path, hash, 3)?;
        }
    }

    Ok(())
}

fn process_natives(
    natives: &Vec<Value>,
    natives_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    for library in natives {
        let library = library.as_object().unwrap();
        let name = library["name"].as_str().unwrap();
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

        if !path.exists() {
            fs::create_dir_all(path.parent().unwrap())?;

            let url = natives["url"].as_str().unwrap();
            try_download_file(url, &path, hash, 3)?;

            // Extract natives jar
            let mut zip = zip::ZipArchive::new(fs::File::open(&path)?)?;

            for i in 0..zip.len() {
                let mut file = zip.by_index(i)?;

                if let Some(exclude) = library["extract"].as_object().unwrap().get("exclude") {
                    let exclude = exclude.as_array().unwrap();
                    let mut excluded = false;

                    for exclude in exclude {
                        if file.name().contains(exclude.as_str().unwrap()) {
                            excluded = true;
                            break;
                        }
                    }

                    if excluded {
                        continue;
                    }
                }

                let path = natives_dir.join(file.name());

                if file.name().ends_with('/') {
                    fs::create_dir_all(path)?;
                } else {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    let mut data = Vec::new();
                    file.read_to_end(&mut data)?;

                    fs::write(path, data)?;
                }
            }
        }
    }

    Ok(())
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
        if path.exists() && !classpath.contains(&path.to_str().unwrap().to_string()) {
            classpath.push(path.to_str().unwrap().to_string());
        }
    }

    classpath
}

impl Launcher {
    /// Install libraries for the current version
    pub fn install_libraries(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.version.profile.is_null() {
            return Err("Please install a version before installing libraries".into());
        }

        let libraries_dir = self.game_dir.join("libraries");
        let natives_dir = self.game_dir.join("natives");

        // Vanilla
        let mut error = None;
        process_libs(
            &self.version.profile["libraries"].as_array().unwrap(),
            &libraries_dir,
            "https://libraries.minecraft.net/",
        )
        .unwrap_or_else(|e| {
            error = Some(e);
        });

        if let Some(e) = error {
            return Err(e);
        }

        // Modded libraries
        if self.version.modded_profile.is_object() {
            let mut error = None;
            process_libs(
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
            .unwrap_or_else(|e| {
                error = Some(e);
            });

            if let Some(e) = error {
                return Err(e);
            }

            if (self.version.forge.enabled && !self.version.forge.legacy)
                || self.version.neoforge.enabled
            {
                process_libs(
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
                .unwrap_or_else(|e| {
                    error = Some(e);
                });

                if let Some(e) = error {
                    return Err(e);
                }

                forge::post_process(
                    &self.game_dir,
                    &self.java_executable,
                    &self.version.forge.install_profile,
                )
                .unwrap_or_else(|e| {
                    error = Some(e);
                });

                if let Some(e) = error {
                    return Err(e);
                }
            }
        }

        // Vanilla
        let mut error = None;
        process_natives(
            &self.version.profile["libraries"].as_array().unwrap(),
            &natives_dir,
        )
        .unwrap_or_else(|e| {
            error = Some(e);
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
