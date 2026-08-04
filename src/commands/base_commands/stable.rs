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

fn stable_latest_pointer_path(stream: &str) -> String {
    format!("{}/latest-{}.txt", stable_dir(), sanitize_name(stream))
}

fn sanitize_name(name: &str) -> String {
    name.replace('/', "_").replace(' ', "_").replace('\n', "_")
}

fn resolve_unique_version_name(base: &str, kind: &str) -> String {
    println!("❌ Error: {} '{}' already exists.", kind, base);
    print!(
        "Enter a different name or press Enter to auto-assign a timestamped name: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let candidate = input.trim();

    if !candidate.is_empty() {
        candidate.to_string()
    } else {
        format!("{}-{}", base, Utc::now().format("%Y%m%d%H%M%S"))
    }
}

fn write_latest_stable_pointer(stream: &str, version: &str) {
    let path = stable_latest_pointer_path(stream);
    let _ = fs::write(&path, version);
}

fn read_latest_stable_pointer(stream: &str) -> Option<String> {
    let path = stable_latest_pointer_path(stream);
    if Path::new(&path).exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            let name = content.trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

pub fn run(command: Option<StableCommands>) {
    if !Path::new(".dam").exists() {
        println!("❌ Error: No reservoir found. Run 'dam source' to initialize one first.");
        return;
    }

    match command {
        Some(StableCommands::Assign { stream, version, description, latest }) => {
            let resolved_stream = stream.unwrap_or_else(|| {
                fs::read_to_string(".dam/CURRENT")
                    .unwrap_or_else(|_| "main".to_string())
                    .trim()
                    .to_string()
            });
            assign_stable(&resolved_stream, &version, description, latest);
        }
        Some(StableCommands::List) => {
            list_stable();
        }
        Some(StableCommands::Inspect { version, latest }) => {
            if latest {
                inspect_latest_stable();
            } else if let Some(v) = version {
                inspect_stable(&v);
            } else {
                list_stable();
            }
        }
        Some(StableCommands::Remove { version }) => {
            remove_stable(&version);
        }
        None => {
            list_stable();
        }
    }
}

fn assign_stable(stream: &str, version: &str, description: Option<String>, mark_latest: bool) {
    fs::create_dir_all(stable_dir()).unwrap_or_default();

    if version.trim().eq_ignore_ascii_case("latest") {
        println!("❌ Error: 'latest' is reserved. Provide a stable version name and use --latest to mark it as the latest pointer.");
        return;
    }

    let mut version_name = version.to_string();
    let mut path = stable_path(&version_name);
    if Path::new(&path).exists() {
        version_name = resolve_unique_version_name(version, "Stable version");
        path = stable_path(&version_name);
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
        version: version_name.clone(),
        stream: stream.to_string(),
        description,
        created_at: Utc::now().to_rfc3339(),
        seal_id,
    };

    if let Ok(json) = serde_json::to_string_pretty(&stable) {
        if fs::write(&path, json).is_ok() {
            println!("✅ Assigned stable version '{}' to stream '{}'.", version_name, stream);
            if mark_latest {
                write_latest_stable_pointer(stream, &version_name);
                println!("✅ Marked '{}' as the latest stable for stream '{}'.", version_name, stream);
            }
            return;
        }
    }

    println!("❌ Error: Failed to save stable version '{}'.", version_name);
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
        println!("{} -> {} ({}) - {}", stable.version, stable.stream, stable.created_at, stable.seal_id);
        if let Some(desc) = stable.description {
            println!("   └─ {}", desc);
        }
    }
    println!();
}

fn inspect_latest_stable() {
    if !Path::new(stable_dir()).exists() {
        println!("📦 No stable versions found.");
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
        println!("📦 No stable versions found.");
        return;
    }

    let current_stream = fs::read_to_string(".dam/CURRENT").unwrap_or_default().trim().to_string();
    let mut filtered: Vec<StableVersion> = stable_versions.into_iter().filter(|s| s.stream == current_stream).collect();
    if filtered.is_empty() {
        println!("No stable versions found for stream '{}'.", current_stream);
        return;
    }

    if let Some(latest_name) = read_latest_stable_pointer(&current_stream) {
        if filtered.iter().any(|s| s.version == latest_name) {
            inspect_stable(&latest_name);
            return;
        }
    }

    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let latest = &filtered[0];
    inspect_stable(&latest.version);
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
