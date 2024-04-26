# Open Launcher

Open Launcher is a package to install and launch modded and vanilla Minecraft instances totally automatically with Rust.

## Note about Java

Java is required to run the game. For the moment, this package cannot download Java for you. The path to the Java executable must be provided to the `Launcher` struct.

## Example usage

```rust
use open_launcher::{auth, version, Launcher};
use std::{env, path};

#[tokio::main]
async fn main() {
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
    )
    .await;

    launcher.auth(auth::OfflineAuth::new("Player"));
    launcher.custom_resolution(1280, 720);
    // launcher.fullscreen(true);
    // launcher.quick_play("multiplayer", "hypixel.net");

    let mut progress = launcher.on_progress();
    tokio::spawn(async move {
        loop {
            match progress.recv().await {
                Ok(progress) => {
                    println!(
                        "Progress: {} {}/{} ({}%)",
                        progress.task,
                        progress.current,
                        progress.total,
                        match progress.total {
                            0 => 0,
                            _ => (progress.current as f64 / progress.total as f64 * 100.0 * 100.0)
                                .round() as u64,
                        } as f32
                            / 100.0
                    );
                }
                Err(_) => {
                    println!("Progress channel closed");
                    break;
                }
            }
        }
    });

    match launcher.install_version().await {
        Ok(_) => println!("Version installed successfully."),
        Err(e) => println!("An error occurred while installing the version: {}", e),
    };

    match launcher.install_assets().await {
        Ok(_) => println!("Assets installed successfully."),
        Err(e) => println!("An error occurred while installing the assets: {}", e),
    };

    match launcher.install_libraries().await {
        Ok(_) => println!("Libraries installed successfully."),
        Err(e) => println!("An error occurred while installing the libraries: {}", e),
    };

    let mut process = match launcher.launch() {
        Ok(p) => p,
        Err(e) => {
            println!("An error occurred while launching the game: {}", e);
            std::process::exit(1);
        }
    };

    let _ = process.wait();

    println!("Game closed.");
}
```

More examples can be found in the [examples](./examples/) directory.

## Documentation

The documentation can be found [here](https://docs.rs/open_launcher).

## License

This project is licensed under the MIT License - see the [LICENSE.md](./LICENSE.md) file for details.
