use serde_json;
use std::error::Error;
use std::path::PathBuf;
use tokio::fs;

use crate::utils::{extract_file, LauncherError};
use crate::Launcher;

pub struct Version {
    pub minecraft_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

pub(crate) struct ForgeVersion {
    pub enabled: bool,
    pub combined: String,
    pub version_path: PathBuf,
    pub install_profile: serde_json::Value,
    pub legacy: bool,
}

pub(crate) struct NeoForgeVersion {
    pub enabled: bool,
    pub combined: String,
    pub version_path: PathBuf,
}

pub(crate) struct FabricVersion {
    pub enabled: bool,
    pub combined: String,
    pub version_path: PathBuf,
}

pub(crate) struct QuiltVersion {
    pub enabled: bool,
    pub combined: String,
    pub version_path: PathBuf,
}

pub(crate) struct InternalVersion {
    pub id: String,
    pub loader_version: String,
    pub profile: serde_json::Value,
    pub modded_profile: serde_json::Value,
    pub forge: ForgeVersion,
    pub neoforge: NeoForgeVersion,
    pub fabric: FabricVersion,
    pub quilt: QuiltVersion,
}

impl InternalVersion {
    pub async fn new(
        game_dir: PathBuf,
        id: String,
        loader: String,
        loader_version: String,
    ) -> Self {
        let mut profile_json = serde_json::Value::Null;
        let mut modded_profile_json = serde_json::Value::Null;
        let mut forge_install_profile_json = serde_json::Value::Null;

        // Vanilla
        let profile_path = game_dir
            .join("versions")
            .join(&id)
            .join(&format!("{}.json", id));
        if profile_path.exists() {
            profile_json =
                serde_json::from_str(&fs::read_to_string(&profile_path).await.unwrap()).unwrap();
        }

        // Forge / NeoForge
        if loader == "forge" {
            let modded_profile_path = game_dir
                .join("versions")
                .join("forge-".to_string() + &format!("{}-{}", id, loader_version.clone()))
                .join(&format!(
                    "forge-{}.json",
                    format!("{}-{}", id, loader_version.clone())
                ));
            if modded_profile_path.exists() {
                modded_profile_json =
                    serde_json::from_str(&fs::read_to_string(&modded_profile_path).await.unwrap())
                        .unwrap();
            }

            let forge_install_profile_path = game_dir
                .join("versions")
                .join("forge-".to_string() + &format!("{}-{}", id, loader_version.clone()))
                .join("install_profile.json");
            if forge_install_profile_path.exists() {
                forge_install_profile_json = serde_json::from_str(
                    &fs::read_to_string(&forge_install_profile_path)
                        .await
                        .unwrap(),
                )
                .unwrap();
            }
        } else if loader == "neoforge" {
            let modded_profile_path = game_dir
                .join("versions")
                .join("neoforge-".to_string() + &loader_version.clone())
                .join(&format!("neoforge-{}.json", loader_version.clone()));
            if modded_profile_path.exists() {
                modded_profile_json =
                    serde_json::from_str(&fs::read_to_string(&modded_profile_path).await.unwrap())
                        .unwrap();
            }

            let forge_install_profile_path = game_dir
                .join("versions")
                .join("neoforge-".to_string() + &loader_version.clone())
                .join("install_profile.json");
            if forge_install_profile_path.exists() {
                forge_install_profile_json = serde_json::from_str(
                    &fs::read_to_string(&forge_install_profile_path)
                        .await
                        .unwrap(),
                )
                .unwrap();
            }
        } else if loader == "fabric" {
            let modded_profile_path = game_dir
                .join("versions")
                .join("fabric-loader-".to_string() + &format!("{}-{}", id, loader_version.clone()))
                .join(&format!(
                    "fabric-loader-{}.json",
                    format!("{}-{}", id, loader_version.clone())
                ));
            if modded_profile_path.exists() {
                modded_profile_json =
                    serde_json::from_str(&fs::read_to_string(&modded_profile_path).await.unwrap())
                        .unwrap();
            }
        } else if loader == "quilt" {
            let modded_profile_path = game_dir
                .join("versions")
                .join("quilt-loader-".to_string() + &loader_version.clone())
                .join(&format!("quilt-loader-{}.json", loader_version.clone()));
            if modded_profile_path.exists() {
                modded_profile_json =
                    serde_json::from_str(&fs::read_to_string(&modded_profile_path).await.unwrap())
                        .unwrap();
            }
        }

        InternalVersion {
            id: id.clone(),
            profile: profile_json,
            loader_version: loader_version.clone(),
            modded_profile: modded_profile_json,
            forge: ForgeVersion {
                enabled: loader == "forge",
                combined: format!("forge-{}-{}", id, loader_version.clone()),
                version_path: game_dir
                    .join("versions")
                    .join("forge-".to_string() + &format!("{}-{}", id, loader_version.clone())),
                install_profile: forge_install_profile_json,
                legacy: if loader == "forge" {
                    let minor = id.split('.').collect::<Vec<&str>>()[1]
                        .parse::<u32>()
                        .unwrap();
                    let patch = id.split('.').collect::<Vec<&str>>()[2]
                        .split('-')
                        .collect::<Vec<&str>>()[0]
                        .parse::<u32>()
                        .unwrap();
                    let forge_patch = loader_version
                        .clone()
                        .split('.')
                        .collect::<Vec<&str>>()
                        .get(3)
                        .unwrap_or(&"0")
                        .parse::<u32>()
                        .unwrap();

                    if minor < 12
                        || (minor == 12 && patch < 2)
                        || (minor == 12 && patch == 2 && forge_patch <= 2847)
                    {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                },
            },
            neoforge: NeoForgeVersion {
                enabled: loader == "neoforge",
                combined: format!("neoforge-{}", loader_version.clone()),
                version_path: game_dir
                    .join("versions")
                    .join("neoforge-".to_string() + &loader_version.clone()),
            },
            fabric: FabricVersion {
                enabled: loader == "fabric",
                combined: format!("fabric-loader-{}-{}", id, loader_version.clone()),
                version_path: game_dir.join("versions").join(
                    "fabric-loader-".to_string() + &format!("{}-{}", id, loader_version.clone()),
                ),
            },
            quilt: QuiltVersion {
                enabled: loader == "quilt",
                combined: format!("quilt-loader-{}", loader_version.clone()),
                version_path: game_dir
                    .join("versions")
                    .join("quilt-loader-".to_string() + &loader_version.clone()),
            },
        }
    }
}

impl Launcher {
    /// Install the selected version
    pub async fn install_version(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        fs::create_dir_all(self.game_dir.join("versions").join(&self.version.id)).await?;

        let _ = self.download_version().await;
        let _ = self.install_modded_versions().await;

        Ok(())
    }

    async fn download_version(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Download version json
        let version_json_path = self
            .game_dir
            .join("versions")
            .join(&self.version.id)
            .join(&format!("{}.json", self.version.id));

        if !version_json_path.exists() {
            let version_manifest_url =
                format!("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json");
            let version_manifest: serde_json::Value =
                reqwest::get(&version_manifest_url).await?.json().await?;
            let version_url = version_manifest["versions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"].as_str().unwrap() == self.version.id)
                .unwrap()["url"]
                .as_str()
                .unwrap();
            let version_json: serde_json::Value = reqwest::get(version_url).await?.json().await?;
            let version_json_str = serde_json::to_string(&version_json)?;
            fs::write(&version_json_path, version_json_str).await?;

            self.version.profile = version_json;
        }

        // Download version jar
        let version_jar_path = self
            .game_dir
            .join("versions")
            .join(&self.version.id)
            .join(&format!("{}.jar", self.version.id));

        if !version_jar_path.exists() {
            let version_json: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&version_json_path).await?)?;
            let version_jar_url = version_json["downloads"]["client"]["url"]
                .as_str()
                .unwrap()
                .to_string();
            let version_jar = reqwest::get(&version_jar_url).await?.bytes().await?;
            fs::write(&version_jar_path, version_jar).await?;
        }

        Ok(())
    }

    async fn install_modded_versions(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Forge / NeoForge
        if self.version.forge.enabled || self.version.neoforge.enabled {
            // Download installer jar
            let forge_installer_path = if self.version.forge.enabled {
                self.version
                    .forge
                    .version_path
                    .join(&format!("{}-installer.jar", self.version.forge.combined))
            } else {
                self.version
                    .neoforge
                    .version_path
                    .join(&format!("{}-installer.jar", self.version.neoforge.combined))
            };

            let forge_installer_url = if self.version.forge.enabled {
                format!(
                    "https://maven.creeperhost.net/net/minecraftforge/forge/{}-{}/{}-installer.jar",
                    self.version.id, self.version.loader_version, self.version.forge.combined
                )
            } else {
                format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/{}-installer.jar",
            self.version.loader_version, self.version.neoforge.combined
        )
            };
            let forge_installer = reqwest::get(&forge_installer_url).await?;

            if !forge_installer.status().is_success() {
                fs::remove_dir_all(self.game_dir.join("versions").join(&self.version.id)).await?;
                self.version.profile = serde_json::Value::Null;
                return Err(Box::from(LauncherError(
                    "Failed to download installer jar".to_string(),
                )));
            }

            if self.version.forge.enabled {
                fs::create_dir_all(&self.version.forge.version_path).await?;
            } else {
                fs::create_dir_all(&self.version.neoforge.version_path).await?;
            }

            fs::write(&forge_installer_path, forge_installer.bytes().await?).await?;

            // Extract installer jar

            // Legacy
            if self.version.forge.legacy {
                // Extract install_profile.json
                extract_file(
                    &forge_installer_path,
                    "install_profile.json",
                    &self.version.forge.version_path,
                )
                .await?;
                let install_profile_json = fs::read_to_string(
                    self.version.forge.version_path.join("install_profile.json"),
                )
                .await?;
                let install_profile_json: serde_json::Value =
                    serde_json::from_str(&install_profile_json)?;

                self.version.modded_profile = install_profile_json["versionInfo"].clone();

                // Extract universal jar
                let universal_jar_path = self
                    .version
                    .forge
                    .version_path
                    .join(&format!("{}.jar", self.version.forge.combined));
                extract_file(
                    &forge_installer_path,
                    install_profile_json["install"]["filePath"]
                        .as_str()
                        .unwrap(),
                    &universal_jar_path,
                )
                .await?;
            } else {
                // Extract data/client.lzma
                extract_file(
                    &forge_installer_path,
                    "data/client.lzma",
                    &self.game_dir.join("data").join("client.lzma"),
                )
                .await?;

                // Extract profile
                let profile_path = if self.version.forge.enabled {
                    self.version
                        .forge
                        .version_path
                        .join(format!("{}.json", self.version.forge.combined))
                } else {
                    self.version
                        .neoforge
                        .version_path
                        .join(format!("{}.json", self.version.neoforge.combined))
                };
                extract_file(&forge_installer_path, "version.json", &profile_path).await?;
                let profile_json = fs::read_to_string(&profile_path).await?;
                let profile_json: serde_json::Value = serde_json::from_str(&profile_json)?;

                // Extract install_profile.json
                let install_profile_path =
                    profile_path.parent().unwrap().join("install_profile.json");
                extract_file(
                    &forge_installer_path,
                    "install_profile.json",
                    &install_profile_path,
                )
                .await?;
                let install_profile_json = fs::read_to_string(&install_profile_path).await?;
                let install_profile_json: serde_json::Value =
                    serde_json::from_str(&install_profile_json)?;

                self.version.modded_profile = profile_json;
                self.version.forge.install_profile = install_profile_json;
            }

            // Remove installer jar
            fs::remove_file(&forge_installer_path).await?;
        }

        // Fabric / Quilt
        if self.version.fabric.enabled || self.version.quilt.enabled {
            let profile_path = if self.version.fabric.enabled {
                self.version
                    .fabric
                    .version_path
                    .join(&format!("{}.json", self.version.fabric.combined))
            } else {
                self.version
                    .quilt
                    .version_path
                    .join(&format!("{}.json", self.version.quilt.combined))
            };
            let profile_url = if self.version.fabric.enabled {
                format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
                    self.version.id, self.version.loader_version
                )
            } else {
                format!(
                    "https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json",
                    self.version.id, self.version.loader_version
                )
            };
            let profile_json: serde_json::Value = reqwest::get(&profile_url).await?.json().await?;

            if self.version.fabric.enabled {
                fs::create_dir_all(&self.version.fabric.version_path).await?;
            } else {
                fs::create_dir_all(&self.version.quilt.version_path).await?;
            }

            fs::write(&profile_path, serde_json::to_string(&profile_json)?).await?;

            self.version.modded_profile = profile_json;
        }
        Ok(())
    }
}
