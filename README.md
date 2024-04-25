# Open Launcher

Open Launcher is a package to install and launch modded and vanilla Minecraft instances totally automatically with Rust.

## Note about Java

Java is required to run the game. For the moment, this package cannot download Java for you. The path to the Java executable must be provided to the `Launcher` struct.

## Example usage

```rust
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
            loader: None,
            loader_version: None,
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
```

More examples can be found in the [examples](./examples/) directory.

## Documentation

The documentation can be found [here](https://docs.rs/open_launcher).

## License

This project is licensed under the MIT License - see the [LICENSE.md](./LICENSE.md) file for details.
