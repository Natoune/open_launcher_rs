use std::{
	collections::HashMap,
	path::PathBuf,
	process::{Child, Command},
};

use tokio::fs;

#[cfg(feature = "blocking")]
pub mod blocking;

pub mod auth;
pub mod version;

mod assets;
mod forge;
mod libraries;
mod utils;

/// The `Launcher` struct is the main struct of the package. It is used to configure and launch a Minecraft game.
pub struct Launcher {
	game_dir: PathBuf,
	game_dir_str: String,
	java_executable: PathBuf,
	version: version::InternalVersion,
	args: Vec<String>,
	game_args: Vec<String>,
	auth: auth::Auth,
	features: HashMap<String, String>,
}

fn process_jvm_args(args: &mut Vec<String>, jvm_args: serde_json::Value) {
	if let Some(jvm_args) = jvm_args.as_array() {
		for jarg in jvm_args {
			if let Some(jarg) = jarg.as_str() {
				arg(args, jarg, true);
			} else {
				let jarg = jarg.as_object().unwrap();
				let rules = jarg.get("rules");
				if !jarg.contains_key("value") {
					continue;
				}
				let value = jarg.get("value").unwrap();

				if rules.is_none() {
					if value.is_string() {
						arg(args, value.as_str().unwrap(), true);
					} else {
						for value in value.as_array().unwrap() {
							arg(args, value.as_str().unwrap(), true);
						}
					}
				} else {
					let mut allowed = false;
					for rule in rules.unwrap().as_array().unwrap() {
						let rule = rule.as_object().unwrap();
						let action = rule["action"].as_str().unwrap();
						let os = rule.get("os");

						if action == "allow" {
							if os.is_none() {
								allowed = true;
							} else {
								let os = os.unwrap().as_object().unwrap();
								if !os.contains_key("name") {
									allowed = true;
								} else {
									let name = os.get("name").unwrap().as_str().unwrap();
									if name == utils::get_os() {
										allowed = true;
									}
								}
							}
						} else if action == "disallow" {
							if os.is_none() {
								allowed = false;
							} else {
								let os = os.unwrap().as_object().unwrap();
								let name = os.get("name").unwrap().as_str().unwrap();
								if name == utils::get_os() {
									allowed = false;
								}
							}
						}
					}

					if allowed {
						if value.is_string() {
							arg(args, value.as_str().unwrap(), true);
						} else {
							for value in value.as_array().unwrap() {
								arg(args, value.as_str().unwrap(), true);
							}
						}
					}
				}
			}
		}
	} else {
		arg(args, "-cp", true);
		arg(args, "${classpath}", true);
	}
}

fn process_game_args(
	args: &mut Vec<String>,
	game_args: serde_json::Value,
	features: HashMap<String, String>,
) {
	if let Some(game_args) = game_args.as_array() {
		for garg in game_args {
			if let Some(garg) = garg.as_str() {
				arg(args, garg, false);
			} else {
				let garg = garg.as_object().unwrap();
				let rules = garg.get("rules");
				let value = garg.get("value").unwrap();

				if rules.is_none() {
					if value.is_string() {
						arg(args, value.as_str().unwrap(), false);
					} else {
						for value in value.as_array().unwrap() {
							arg(args, value.as_str().unwrap(), false);
						}
					}
				} else {
					let mut allowed = false;
					for rule in rules.unwrap().as_array().unwrap() {
						let rule = rule.as_object().unwrap();
						let action = rule["action"].as_str().unwrap();
						let cond_features = rule.get("features");
						let os = rule.get("os");

						if action == "allow" {
							if !cond_features.is_none() {
								let cond_features = cond_features.unwrap().as_object().unwrap();
								let mut passed = true;
								for (key, value) in cond_features {
									if features.contains_key(key) {
										if &features[key]
											!= serde_json::to_string(value).unwrap().as_str()
										{
											passed = false;
										}
									} else {
										passed = false;
									}
								}
								if passed {
									allowed = true;
								}
							} else if os.is_none() {
								allowed = true;
							} else if !os.is_none() {
								let os = os.unwrap().as_object().unwrap();
								let name = os.get("name").unwrap().as_str().unwrap();
								if name == utils::get_os() {
									allowed = true;
								}
							} else {
								allowed = true;
							}
						} else if action == "disallow" {
							if os.is_none() {
								allowed = false;
							} else {
								let os = os.unwrap().as_object().unwrap();
								let name = os.get("name").unwrap().as_str().unwrap();
								if name == utils::get_os() {
									allowed = false;
								}
							}
						}
					}

					if allowed {
						if value.is_string() {
							arg(args, value.as_str().unwrap(), false);
						} else {
							for value in value.as_array().unwrap() {
								arg(args, value.as_str().unwrap(), false);
							}
						}
					}
				}
			}
		}
	}
}

fn process_legacy_game_args(args: &mut Vec<String>, game_args: String) {
	let game_args = game_args.split(' ').collect::<Vec<&str>>();
	for garg in game_args {
		arg(args, garg, false);
	}
}

fn arg(args: &mut Vec<String>, arg: &str, ignore_checks: bool) {
	if ignore_checks {
		args.push(arg.trim().to_string());
	} else {
		for arg in arg.split(' ') {
			if !args.clone().contains(&arg.trim().to_string()) {
				args.push(arg.trim().to_string());
			}
		}
	}
}

impl Launcher {
	/// Create a new `Launcher` instance.
	/// # Arguments
	/// * `game_dir` - The directory where the game files will be stored.
	/// * `java_executable` - The path to the Java executable (e.g. `java` for linux, `java.exe` for windows).
	/// * `version` - The version of Minecraft to launch. (`version::Version` struct)
	/// # Example
	/// ```
	/// use open_launcher::{auth, version, Launcher};
	/// let mut launcher = Launcher::new(
	///     "/home/user/.open_launcher",
	///     "/usr/bin/java",
	///     version::Version {
	///         minecraft_version: "1.20.2".to_string(),
	///         loader: Some("quilt".to_string()),
	///         loader_version: Some("0.25.0".to_string()),
	///     }
	/// );
	/// ```
	pub async fn new(game_dir: &str, java_executable: &str, version: version::Version) -> Self {
		let game_dir = game_dir.replace("/", std::path::MAIN_SEPARATOR_STR);
		let game_dir = std::path::Path::new(&game_dir);
		fs::create_dir_all(&game_dir).await.unwrap();

		let java_executable = java_executable.replace("/", std::path::MAIN_SEPARATOR_STR);
		let java_executable = std::path::Path::new(&java_executable);

		Launcher {
			game_dir: game_dir.to_path_buf(),
			game_dir_str: game_dir.to_str().unwrap().to_string(),
			java_executable: java_executable.to_path_buf(),
			version: version::InternalVersion::new(
				game_dir.to_path_buf(),
				version.minecraft_version,
				version.loader.unwrap_or("vanilla".to_string()),
				version.loader_version.unwrap_or("".to_string()),
			)
			.await,
			args: Vec::new(),
			game_args: Vec::new(),
			auth: auth::Auth::default(),
			features: HashMap::new(),
		}
	}

	/// Add a jvm argument to the launch command.
	/// # Arguments
	/// * `arg` - The argument to add.
	/// # Example
	/// ```
	/// launcher.jvm_arg("-Xmx2G");
	/// ```
	pub fn jvm_arg(&mut self, arg: &str) {
		self.args.push(arg.to_string());
	}

	/// Add a game argument to the launch command.
	/// # Arguments
	/// * `arg` - The argument to add.
	/// # Example
	/// ```
	/// launcher.arg("--demo");
	/// ```
	pub fn arg(&mut self, arg: &str) {
		self.game_args.push(arg.to_string());
	}

	/// Set the authentication details.
	/// # Arguments
	/// * `auth` - The authentication details.
	/// # Example
	/// Online auth:
	/// ```
	/// launcher.auth(auth::Auth::new(
	///     username: "username".to_string(),
	///     uuid: "uuid".to_string(),
	///     access_token: "access_token".to_string(),
	///     user_type: "msa".to_string(),
	///     user_properties: "{}".to_string(),
	/// ));
	/// ```
	/// # Example
	/// Offline auth:
	/// ```
	/// launcher.auth(auth::OfflineAuth::new("username"));
	/// ```
	pub fn auth(&mut self, auth: auth::Auth) {
		self.auth = auth;
	}

	pub fn demo_user(&mut self, demo_user: bool) {
		self.features
			.insert("is_demo_user".to_string(), demo_user.to_string());
	}

	pub fn custom_resolution(&mut self, width: i32, height: i32) {
		self.features
			.insert("has_custom_resolution".to_string(), "true".to_string());
		self.features
			.insert("resolution_width".to_string(), width.to_string());
		self.features
			.insert("resolution_height".to_string(), height.to_string());
	}

	pub fn fullscreen(&mut self, fullscreen: bool) {
		self.features
			.insert("fullscreen".to_string(), fullscreen.to_string());
	}

	pub fn quick_play(&mut self, quick_play: &str, value: &str) {
		if quick_play == "path" {
			self.features
				.insert("has_quick_plays_support".to_string(), "true".to_string());
			self.features
				.insert("quickPlayPath".to_string(), value.to_string());
		} else if quick_play == "singleplayer" {
			self.features
				.insert("is_quick_play_singleplayer".to_string(), "true".to_string());
			self.features
				.insert("quickPlaySingleplayer".to_string(), value.to_string());
		} else if quick_play == "multiplayer" {
			self.features
				.insert("is_quick_play_multiplayer".to_string(), "true".to_string());
			self.features
				.insert("quickPlayMultiplayer".to_string(), value.to_string());
		} else if quick_play == "realms" {
			self.features
				.insert("is_quick_play_realms".to_string(), "true".to_string());
			self.features
				.insert("quickPlayRealms".to_string(), value.to_string());
		}
	}

	/// Get the command to launch the game.
	/// # Returns
	/// * `Result<Command, Box<dyn std::error::Error>>` - The command to launch the game.
	/// # Example
	/// ```
	/// let command = launcher.command().unwrap();
	/// ```
	pub fn command(&mut self) -> Result<Command, Box<dyn std::error::Error>> {
		if self.version.profile.is_null() {
			return Err("Please install a version before launching".into());
		}

		println!("Launching Minecraft version {}", self.version.id);

		let mut args = self.args.clone();

		let classpath_separator = match std::env::consts::OS {
			"windows" => ";",
			_ => ":",
		};

		let mut classpath = self.get_classpath();
		classpath.push(
			self.game_dir
				.join("versions")
				.join(&self.version.id)
				.join(&format!("{}.jar", self.version.id))
				.to_str()
				.unwrap()
				.to_string(),
		);

		if self.version.forge.enabled {
			let universal_jar_path = self
				.version
				.forge
				.version_path
				.join(format!("{}.jar", self.version.forge.combined));
			if universal_jar_path.exists() {
				classpath.push(universal_jar_path.to_str().unwrap().to_string());
			}
		} else if self.version.neoforge.enabled {
			let universal_jar_path = self
				.version
				.neoforge
				.version_path
				.join(format!("{}.jar", self.version.neoforge.combined));
			if universal_jar_path.exists() {
				classpath.push(universal_jar_path.to_str().unwrap().to_string());
			}
		}

		// JVM args
		process_jvm_args(&mut args, self.version.profile["arguments"]["jvm"].clone());
		if self.version.modded_profile.is_object()
			&& self
				.version
				.modded_profile
				.as_object()
				.unwrap()
				.contains_key("arguments")
		{
			process_jvm_args(
				&mut args,
				self.version.modded_profile["arguments"]["jvm"].clone(),
			);
		}

		// Misc
		arg(&mut args, "-XX:-UseAdaptiveSizePolicy", false);
		arg(&mut args, "-XX:-OmitStackTraceInFastThrow", false);
		arg(
			&mut args,
			"-Dfml.ignoreInvalidMinecraftCertificates=true",
			false,
		);
		arg(&mut args, "-Dfml.ignorePatchDiscrepancies=true", false);

		// Natives
		arg(
			&mut args,
			&format!("-Djava.library.path=${{natives_directory}}",),
			false,
		);

		// Main class
		if self.version.modded_profile.is_object() {
			arg(
				&mut args,
				self.version.modded_profile["mainClass"].as_str().unwrap(),
				false,
			);
		} else {
			arg(
				&mut args,
				self.version.profile["mainClass"].as_str().unwrap(),
				false,
			);
		}

		// Game args
		for garg in self.game_args.clone() {
			arg(&mut args, garg.as_str(), false);
		}

		if self
			.version
			.profile
			.as_object()
			.unwrap()
			.contains_key("minecraftArguments")
		{
			// LEGACY
			if self.version.modded_profile.is_object()
				&& self
					.version
					.modded_profile
					.as_object()
					.unwrap()
					.contains_key("minecraftArguments")
			{
				process_legacy_game_args(
					&mut args,
					self.version.modded_profile["minecraftArguments"]
						.as_str()
						.unwrap()
						.to_string(),
				);
			} else {
				process_legacy_game_args(
					&mut args,
					self.version.profile["minecraftArguments"]
						.as_str()
						.unwrap()
						.to_string(),
				);
			}
		} else {
			process_game_args(
				&mut args,
				self.version.profile["arguments"]["game"].clone(),
				self.features.clone(),
			);

			if self.version.modded_profile.is_object()
				&& self
					.version
					.modded_profile
					.as_object()
					.unwrap()
					.contains_key("arguments")
			{
				process_game_args(
					&mut args,
					self.version.modded_profile["arguments"]["game"].clone(),
					self.features.clone(),
				);
			}
		}

		let mut fields = self.features.clone();
		fields.insert("classpath".to_string(), classpath.join(classpath_separator));
		fields.insert(
			"classpath_separator".to_string(),
			classpath_separator.to_string(),
		);
		fields.insert(
			"natives_directory".to_string(),
			if self.version.id.split('.').collect::<Vec<&str>>()[1]
				.parse::<i32>()
				.unwrap() >= 19
			{
				self.game_dir.to_str().unwrap().to_string()
			} else {
				self.game_dir.join("natives").to_str().unwrap().to_string()
			},
		);
		fields.insert(
			"library_directory".to_string(),
			self.game_dir
				.clone()
				.join("libraries")
				.to_str()
				.unwrap()
				.to_string(),
		);
		fields.insert("launcher_name".to_string(), "open_launcher".to_string());
		fields.insert(
			"launcher_version".to_string(),
			self.version.profile["minimumLauncherVersion"]
				.as_str()
				.unwrap_or(env!("CARGO_PKG_VERSION"))
				.to_string(),
		);
		fields.insert("auth_player_name".to_string(), self.auth.username.clone());
		fields.insert("version_name".to_string(), self.version.id.clone());
		fields.insert("game_directory".to_string(), self.game_dir_str.clone());
		fields.insert(
			"assets_root".to_string(),
			self.game_dir.join("assets").to_str().unwrap().to_string(),
		);
		fields.insert(
			"assets_index_name".to_string(),
			self.version.profile["assets"].as_str().unwrap().to_string(),
		);
		fields.insert("auth_uuid".to_string(), self.auth.uuid.clone());
		fields.insert(
			"auth_access_token".to_string(),
			self.auth.access_token.clone(),
		);
		fields.insert("user_type".to_string(), self.auth.user_type.clone());
		fields.insert(
			"version_type".to_string(),
			self.version.profile["type"].as_str().unwrap().to_string(),
		);
		fields.insert(
			"user_properties".to_string(),
			self.auth.user_properties.clone(),
		);
		fields.insert(
			"game_assets".to_string(),
			if self.version.profile["assets"] == "legacy"
				|| self.version.profile["assets"] == "pre-1.6"
			{
				self.game_dir
					.join("resources")
					.to_str()
					.unwrap()
					.to_string()
			} else {
				self.game_dir.join("assets").to_str().unwrap().to_string()
			},
		);
		fields.insert("auth_session".to_string(), self.auth.access_token.clone());
		fields.insert("clientid".to_string(), "0".to_string());
		fields.insert("auth_xuid".to_string(), "0".to_string());

		// Replace fields
		let mut final_args = vec![];

		for arg in args {
			let mut arg = arg.clone();
			for (key, value) in &fields {
				arg = arg.replace(&format!("${{{}}}", key), value.as_str());
			}
			final_args.push(arg);
		}

		// Features
		if self.features.contains_key("is_demo_user") && self.features["is_demo_user"] == "true" {
			arg(&mut final_args, "--demo", false);
		}
		if self.features.contains_key("has_custom_resolution") {
			arg(
				&mut final_args,
				&format!(
					"--width {} --height {}",
					self.features["resolution_width"], self.features["resolution_height"]
				),
				false,
			);
		}
		if self.features.contains_key("fullscreen") && self.features["fullscreen"] == "true" {
			arg(&mut final_args, "--fullscreen", false);
		}
		if self.features.contains_key("has_quick_plays_support") {
			arg(
				&mut final_args,
				&format!("--quickPlayPath {}", self.features["quickPlayPath"]),
				false,
			);
		}
		if self.features.contains_key("is_quick_play_singleplayer") {
			arg(
				&mut final_args,
				&format!(
					"--quickPlaySingleplayer {}",
					self.features["quickPlaySingleplayer"]
				),
				false,
			);
		}
		if self.features.contains_key("is_quick_play_multiplayer") {
			arg(
				&mut final_args,
				&format!(
					"--quickPlayMultiplayer {}",
					self.features["quickPlayMultiplayer"]
				),
				false,
			);
		}
		if self.features.contains_key("is_quick_play_realms") {
			arg(
				&mut final_args,
				&format!("--quickPlayRealms {}", self.features["quickPlayRealms"]),
				false,
			);
		}

		// Command
		let mut command = Command::new(&self.java_executable);
		command.args(final_args);
		command.current_dir(self.game_dir.clone());

		Ok(command)
	}

	/// Launch the game.
	/// # Returns
	/// * `Result<Child, Box<dyn std::error::Error>>` - The child process of the game.
	pub fn launch(&mut self) -> Result<Child, Box<dyn std::error::Error>> {
		let mut command: Command = self.command()?;

		println!("Running command: {:?}", command);

		Ok(command.spawn().unwrap())
	}
}
