use crate::{
    events,
    libraries::{get_lib_path, get_libraries_classpath},
};
use async_process::Command;
use serde_json::{Map, Value};
use sha1::Digest;
use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{BufRead, Read},
    path::PathBuf,
};
use tokio::sync::broadcast;

fn normalize_variable(val: &str, fields: &HashMap<String, String>) -> String {
    let mut val = val.to_string();
    for (key, value) in fields.iter() {
        val = val.replace(&format!("{{{}}}", key), value);
    }
    val
}

fn resolve_outputs(
    proc: &Map<String, Value>,
    fields: &HashMap<String, String>,
) -> Map<String, Value> {
    let mut outputs = match proc.clone().get("outputs") {
        Some(outputs) => outputs.as_object().unwrap().clone(),
        None => Map::new(),
    };
    for (_, value) in outputs.iter_mut() {
        *value = serde_json::Value::String(normalize_variable(value.as_str().unwrap(), fields));
    }

    let args = proc["args"].as_array().unwrap();
    for i in 0..args.len() {
        let arg = args[i].as_str().unwrap();
        if arg == "--output" || arg == "--out-jar" {
            let path = normalize_variable(args[i + 1].as_str().unwrap(), fields);
            outputs.insert(path, "".into());
        }
    }

    outputs.clone()
}

fn check_outputs(
    proc: &Map<String, Value>,
    game_dir: &PathBuf,
    fields: &HashMap<String, String>,
) -> bool {
    let sides = match proc.get("sides") {
        Some(sides) => Some(sides.as_array().unwrap()),
        None => None,
    };

    if sides.is_some()
        && !sides
            .unwrap()
            .contains(&Value::String("client".to_string()))
    {
        return true;
    }

    let outputs = resolve_outputs(proc, &fields);

    let mut valid = true;

    for (path, sha) in outputs {
        let mut path = path.to_string();
        for (key, value) in fields.iter() {
            path = path.replace(&format!("{{{}}}", key), &value);
        }
        let path = game_dir.join("libraries").join(path);
        let mut sha = sha.as_str().unwrap().to_string();
        for (key, value) in fields.iter() {
            sha = sha.replace(&format!("{{{}}}", key), &value);
        }

        if !path.exists()
            || (sha.len() == 40
                && format!("{:x}", sha1::Sha1::digest(&fs::read(path).unwrap())) != sha)
        {
            valid = false;
            break;
        }
    }

    valid
}

pub(crate) async fn post_process(
    game_dir: &PathBuf,
    java_executable: &PathBuf,
    install_profile: &Value,
    progress_sender: broadcast::Sender<events::Progress>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let data = install_profile["data"].as_object().unwrap();
    let processors = install_profile["processors"].as_array().unwrap();

    let mut fields: HashMap<String, String> = HashMap::new();
    fields.insert("SIDE".to_string(), "client".into());
    fields.insert(
        "MINECRAFT_JAR".to_string(),
        game_dir
            .join("versions")
            .join(install_profile["minecraft"].as_str().unwrap())
            .join(format!(
                "{}.jar",
                install_profile["minecraft"].as_str().unwrap()
            ))
            .to_str()
            .unwrap()
            .into(),
    );
    fields.insert(
        "ROOT".to_string(),
        game_dir.clone().to_str().unwrap().into(),
    );
    fields.insert(
        "MINECRAFT_VERSION".to_string(),
        install_profile["minecraft"].as_str().unwrap().into(),
    );
    fields.insert(
        "LIBRARY_DIR".to_string(),
        game_dir.join("libraries").to_str().unwrap().into(),
    );

    for (key, value) in data {
        let key = key.as_str();
        let client = value["client"].as_str().unwrap();

        if client.starts_with('[') && client.ends_with(']') {
            let client = client.trim_start_matches('[').trim_end_matches(']');
            let client = serde_json::Value::String(client.to_string());

            fields.insert(
                key.to_string(),
                game_dir
                    .join("libraries")
                    .join(get_lib_path(client.as_str().unwrap()))
                    .to_str()
                    .unwrap()
                    .into(),
            );
        } else if client.starts_with('\'') && client.ends_with('\'') {
            fields.insert(key.to_string(), client.trim_matches('\'').into());
        } else if client.starts_with("/data/") {
            fields.insert(
                key.to_string(),
                game_dir
                    .join(client.trim_start_matches('/'))
                    .to_str()
                    .unwrap()
                    .into(),
            );
        } else {
            fields.insert(key.to_string(), client.into());
        }
    }

    let mut skip = true;
    for proc in processors {
        let proc = proc.as_object().unwrap();
        if !check_outputs(proc, game_dir, &fields) {
            skip = false;
            break;
        }
    }

    if skip {
        return Ok(());
    }

    let _ = progress_sender.send(events::Progress {
        task: "post_processing".to_string(),
        file: String::new(),
        total: processors.len() as u64,
        current: 0,
    });

    let mut i = 0;
    for proc in processors {
        let proc = proc.as_object().unwrap();
        let args = proc["args"].as_array().unwrap();
        let classpath = proc["classpath"].as_array().unwrap();
        let jar = proc["jar"].as_str().unwrap();
        let sides = match proc.get("sides") {
            Some(sides) => Some(sides.as_array().unwrap()),
            None => None,
        };

        if sides.is_some()
            && !sides
                .unwrap()
                .contains(&Value::String("client".to_string()))
        {
            continue;
        }

        // Add processor jar to classpath
        let mut classpath = classpath.clone();
        classpath.push(serde_json::Value::String(jar.to_string()));

        // Find main class from jar manifest
        let main_class = {
            let jar_path = game_dir.join("libraries").join(get_lib_path(jar));
            let mut jar = zip::ZipArchive::new(std::fs::File::open(jar_path)?)?;
            let manifest = jar.by_name("META-INF/MANIFEST.MF")?;
            let mut main_class = None;
            for line in std::io::BufReader::new(manifest).by_ref().lines() {
                let line = line?;
                if line.starts_with("Main-Class: ") {
                    main_class = Some(line.trim_start_matches("Main-Class: ").to_string());
                    break;
                }
            }
            main_class.unwrap()
        };

        // Run the processor
        let mut command = Command::new(java_executable);
        command.arg("-cp");
        command.arg(get_libraries_classpath(game_dir, &classpath).join(
            match std::env::consts::OS {
                "windows" => ";",
                _ => ":",
            },
        ));
        command.arg(main_class.clone());

        for arg in args {
            let arg = arg.as_str().unwrap();
            let mut arg = arg.to_string();

            // Replace fields in args
            for (key, value) in fields.iter() {
                arg = arg.replace(&format!("{{{}}}", key), value);
            }

            // Check for library references
            if arg.starts_with("[") && arg.ends_with("]") {
                let arg = arg.trim_start_matches('[').trim_end_matches(']');
                let arg = serde_json::Value::String(arg.to_string());
                let arg = game_dir
                    .join("libraries")
                    .join(get_lib_path(arg.as_str().unwrap()));
                command.arg(arg);
                continue;
            }

            command.arg(arg);
        }

        command.current_dir(game_dir);
        let mut process = command.spawn()?;
        let status = process.status().await?;
        if !status.success() {
            return Err("Processor failed".into());
        }

        // Check outputs
        if !check_outputs(proc, game_dir, &fields) {
            return Err("Processor failed".into());
        }

        i += 1;
        let _ = progress_sender.send(events::Progress {
            task: "post_processing".to_string(),
            file: main_class,
            total: processors.len() as u64,
            current: i,
        });
    }

    Ok(())
}
