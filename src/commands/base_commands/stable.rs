use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use crate::cli::StableCommands;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StableVersion {
    pub version: String,
    pub stream: String,
    pub description: Option<String>,
    pub created_at: String,
    pub seal_id: String,
}

fn stable_dir() -> &'static str {
    ".dam/stable_versions"
}

fn stable_path(version: &str) -> String {
    format!("{}/{}.json", stable_dir(), sanitize_name(version))
}

fn sanitize_name(name: &str) -> String {
    name.replace('/', "_").replace(' ', "_").replace('\n', "_")
}

pub fn run(command: Option<StableCommands>) {
    if !Path::new(".dam").exists() {
        println!("❌ Error: No reservoir found. Run 'dam source' to initialize one first.");
        return;
    }

    match command {
        Some(StableCommands::Assign { stream, version, description }) => {
            assign_stable(&stream, &version, description);
        }
        Some(StableCommands::List) => {
            list_stable();
        }
        Some(StableCommands::Inspect { version }) => {
            inspect_stable(&version);
        }
        Some(StableCommands::Remove { version }) => {
            remove_stable(&version);
        }
        None => {
            list_stable();
        }
    }
}

fn assign_stable(stream: &str, version: &str, description: Option<String>) {
    fs::create_dir_all(stable_dir()).unwrap_or_default();

    let path = stable_path(version);
    if Path::new(&path).exists() {
        println!("❌ Error: Stable version '{}' already exists. Use a different version or remove the old one first.", version);
        return;
    }

    let stream_meta_path = format!(".dam/streams/{}", stream);
    if !Path::new(&stream_meta_path).exists() {
        println!("❌ Error: Stream '{}' does not exist. Create it first with 'dam stream create {}'.", stream, stream);
        return;
    }

    let seal_id = crate::commands::stream::get_or_create_meta(stream)
        .latest_seal
        .unwrap_or_else(|| {
            println!("❌ Error: Stream '{}' has no latest seal. Seal the stream first.", stream);
            std::process::exit(1);
        });

    let stable = StableVersion {
        version: version.to_string(),
        stream: stream.to_string(),
        description,
        created_at: Utc::now().to_rfc3339(),
        seal_id,
    };

    if let Ok(json) = serde_json::to_string_pretty(&stable) {
        if fs::write(&path, json).is_ok() {
            println!("✅ Assigned stable version '{}' to stream '{}'.", version, stream);
            return;
        }
    }

    println!("❌ Error: Failed to save stable version '{}'.", version);
}

fn list_stable() {
    if !Path::new(stable_dir()).exists() {
        println!("📦 No stable versions found. Assign one with: dam stable assign <stream> <version>");
        return;
    }

    let entries = match fs::read_dir(stable_dir()) {
        Ok(e) => e,
        Err(_) => {
            println!("📦 No stable versions found.");
            return;
        }
    };

    let mut stable_versions: Vec<StableVersion> = Vec::new();
    for entry in entries.flatten() {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            if let Ok(stable) = serde_json::from_str::<StableVersion>(&content) {
                stable_versions.push(stable);
            }
        }
    }

    if stable_versions.is_empty() {
        println!("📦 No stable versions found. Assign one with: dam stable assign <stream> <version>");
        return;
    }

    stable_versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    println!("\n📌 Stable Stream Versions:");
    println!("─────────────────────────────────────");
    for stable in stable_versions {
        println!("{} -> {} ({})", stable.version, stable.stream, stable.created_at);
        if let Some(desc) = stable.description {
            println!("   └─ {}", desc);
        }
    }
    println!();
}

fn inspect_stable(version: &str) {
    let path = stable_path(version);
    if !Path::new(&path).exists() {
        println!("❌ Error: Stable version '{}' not found.", version);
        return;
    }

    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(stable) = serde_json::from_str::<StableVersion>(&content) {
            println!("\n📌 Stable Version: {}", stable.version);
            println!("─────────────────────────────────────");
            println!("Stream      : {}", stable.stream);
            println!("Seal ID     : {}", stable.seal_id);
            println!("Created At  : {}", stable.created_at);
            if let Some(desc) = stable.description {
                println!("Description : {}", desc);
            }
            println!();
            return;
        }
    }
    println!("❌ Error: Failed to read stable version '{}'.", version);
}

fn remove_stable(version: &str) {
    let path = stable_path(version);
    if !Path::new(&path).exists() {
        println!("❌ Error: Stable version '{}' not found.", version);
        return;
    }

    print!("⚠️  Are you sure you want to remove stable version '{}' ? (y/N): ", version);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Aborted.");
        return;
    }

    if fs::remove_file(&path).is_ok() {
        println!("✅ Removed stable version '{}'.", version);
    } else {
        println!("❌ Error: Failed to remove stable version '{}'.", version);
    }
}
