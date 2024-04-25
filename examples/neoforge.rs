use open_launcher::blocking::{auth, version, Launcher};
use std::{env, path};

fn main() {
    let mut launcher = Launcher::new(
        path::Path::new(env::home_dir().unwrap().as_path())
            .join(".open_launcher")
            .to_str()
            .unwrap(),
        path::Path::new(env::home_dir().unwrap().as_path())
            .join(".open_launcher")
            .join("jre")
            .join("bin")
            .join("java.exe")
            .to_str()
            .unwrap(),
        version::Version {
            minecraft_version: "1.20.2".to_string(),
            loader: Some("neoforge".to_string()),
            loader_version: Some("20.2.88".to_string()),
        },
    );

    launcher.auth(auth::OfflineAuth::new("Player"));
    launcher.custom_resolution(1280, 720);
    // launcher.fullscreen(true);
    // launcher.quick_play("multiplayer", "hypixel.net");

    launcher.install_version().unwrap_or_else(|e| {
        println!("An error occurred while installing the version: {}", e);
    });

    launcher.install_assets().unwrap_or_else(|e| {
        println!("An error occurred while installing the assets: {}", e);
    });

    launcher.install_libraries().unwrap_or_else(|e| {
        println!("An error occurred while installing the libraries: {}", e);
    });

    let mut process = launcher.launch().unwrap_or_else(|e| {
        println!("An error occurred while launching the game: {}", e);
        std::process::exit(1);
    });

    let _ = process.wait();

    println!("Game closed.");
}
