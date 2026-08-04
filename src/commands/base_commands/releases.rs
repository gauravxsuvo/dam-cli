use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use chrono::Utc;
use crate::cli::ReleasesCommands;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Release {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub seal_id: String,
    pub stream: String,
}

fn releases_dir() -> &'static str {
    ".dam/releases"
}

fn release_path(name: &str) -> String {
    format!("{}/{}.json", releases_dir(), sanitize_release_name(name))
}

fn release_latest_pointer_path() -> &'static str {
    ".dam/releases/LATEST"
}

fn sanitize_release_name(name: &str) -> String {
    name.replace('/', "_").replace(' ', "_").replace('\n', "_")
}

fn resolve_unique_release_name(base: &str) -> String {
    println!("❌ Error: Release '{}' already exists.", base);
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

fn write_latest_release_pointer(name: &str) {
    let _ = fs::write(release_latest_pointer_path(), name);
}

fn read_latest_release_pointer() -> Option<String> {
    let path = release_latest_pointer_path();
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

pub fn run(command: Option<ReleasesCommands>) {
    if !Path::new(".dam").exists() {
        println!("❌ Error: No reservoir found. Run 'dam source' to initialize one first.");
        return;
    }

    match command {
        Some(ReleasesCommands::Create { name, stream, description, tags, latest }) => {
            create_release(&name, stream.as_deref(), description, tags, latest);
        }
        Some(ReleasesCommands::List) => {
            list_releases();
        }
        Some(ReleasesCommands::Inspect { name, latest }) => {
            if latest {
                if let Some(latest_name) = read_latest_release_pointer() {
                    inspect_release(&latest_name);
                } else {
                    println!("📦 No latest release pointer found.");
                }
            } else if let Some(name) = name {
                inspect_release(&name);
            } else {
                println!("❌ Error: Please provide a release name or use --latest.");
            }
        }
        Some(ReleasesCommands::Delete { name }) => {
            delete_release(&name);
        }
        None => {
            list_releases();
        }
    }
}

fn create_release(name: &str, stream_opt: Option<&str>, description: Option<String>, tags: Option<String>, mark_latest: bool) {
    // Ensure releases directory exists
    fs::create_dir_all(releases_dir()).unwrap_or_default();

    if name.trim().eq_ignore_ascii_case("latest") {
        println!("❌ Error: 'latest' is reserved. Provide a real release name and use --latest to mark it as current.");
        return;
    }

    let mut release_name = name.to_string();
    let mut path = release_path(&release_name);
    while Path::new(&path).exists() {
        release_name = resolve_unique_release_name(&release_name);
        path = release_path(&release_name);
    }

    let current_stream = stream_opt
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            fs::read_to_string(".dam/CURRENT")
                .unwrap_or_else(|_| "main".to_string())
                .trim()
                .to_string()
        });
    let stream_meta = crate::commands::stream::get_or_create_meta(&current_stream);

    let latest_seal = match stream_meta.latest_seal {
        Some(s) => s,
        None => {
            println!("❌ Error: No seals found in stream '{}'. Create a seal first before releasing.", current_stream);
            return;
        }
    };

    let parsed_tags: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let release = Release {
        name: release_name.clone(),
        version: release_name.clone(),
        description,
        tags: parsed_tags,
        created_at: Utc::now().to_rfc3339(),
        seal_id: latest_seal.clone(),
        stream: current_stream.clone(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&release) {
        if fs::write(&path, json).is_ok() {
            println!("✅ Created release '{}' from seal '{}' on stream '{}'.", release_name, latest_seal, current_stream);
            if let Some(desc) = &release.description {
                println!("   Description: {}", desc);
            }
            if !release.tags.is_empty() {
                println!("   Tags: {}", release.tags.join(", "));
            }
            if mark_latest {
                write_latest_release_pointer(&release_name);
                println!("✅ Marked '{}' as the latest release.", release_name);
            }
            return;
        }
    }
    println!("❌ Error: Failed to save release '{}'", release_name);
}

fn list_releases() {
    if !Path::new(releases_dir()).exists() {
        println!("📦 No releases found. Create one with: dam releases create <name>");
        return;
    }

    let entries = match fs::read_dir(releases_dir()) {
        Ok(e) => e,
        Err(_) => {
            println!("📦 No releases found.");
            return;
        }
    };

    let mut releases: Vec<Release> = Vec::new();
    for entry in entries.flatten() {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            if let Ok(release) = serde_json::from_str::<Release>(&content) {
                releases.push(release);
            }
        }
    }

    if releases.is_empty() {
        println!("📦 No releases found. Create one with: dam releases create <name>");
        return;
    }

    releases.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    println!("\n📦 Available Releases:");
    println!("─────────────────────────────────────");
    for (idx, release) in releases.iter().enumerate() {
        println!("{} release: {} ({})", idx + 1, release.name, release.created_at);
        if let Some(desc) = &release.description {
            println!("   └─ {}", desc);
        }
        if !release.tags.is_empty() {
            println!("   └─ tags: {}", release.tags.join(", "));
        }
    }
    println!();
}

fn inspect_release(name: &str) {
    let path = release_path(name);
    if !Path::new(&path).exists() {
        println!("❌ Error: Release '{}' not found.", name);
        return;
    }

    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(release) = serde_json::from_str::<Release>(&content) {
            println!("\n📦 Release Details: {}", release.name);
            println!("─────────────────────────────────────");
            println!("Version      : {}", release.version);
            println!("Stream       : {}", release.stream);
            println!("Seal ID      : {}", release.seal_id);
            println!("Created At   : {}", release.created_at);
            if let Some(desc) = &release.description {
                println!("Description  : {}", desc);
            }
            if !release.tags.is_empty() {
                println!("Tags         : {}", release.tags.join(", "));
            }
            println!();
            return;
        }
    }
    println!("❌ Error: Failed to read release '{}'.", name);
}

fn delete_release(name: &str) {
    let path = release_path(name);
    if !Path::new(&path).exists() {
        println!("❌ Error: Release '{}' not found.", name);
        return;
    }

    print!("⚠️  Are you sure you want to delete release '{}'? (y/N): ", name);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Aborted.");
        return;
    }

    if fs::remove_file(&path).is_ok() {
        println!("✅ Deleted release '{}'", name);
    } else {
        println!("❌ Error: Failed to delete release '{}'", name);
    }
}

pub fn sync_releases(
    release_name: Option<String>,
    action: Option<String>,
    platform_arg: Option<String>,
    _force: bool,
) {
    let provider_name = platform_arg.unwrap_or_else(|| "github".to_string());
    println!("\n--- [DAM RELEASE SYNC: {}] ---", provider_name.to_uppercase());

    // Ensure releases directory exists
    if !Path::new(releases_dir()).exists() {
        println!("❌ Error: No releases found. Create one with: dam releases create <name>");
        return;
    }

    let releases_to_sync: Vec<String> = if let Some(rn) = release_name {
        let path = release_path(&rn);
        if !Path::new(&path).exists() {
            println!("❌ Error: Release '{}' does not exist.", rn);
            return;
        }
        vec![rn]
    } else {
        // List all local releases
        let mut releases = Vec::new();
        if let Ok(entries) = fs::read_dir(releases_dir()) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        releases.push(name[..name.len() - 5].to_string());
                    }
                }
            }
        }
        if releases.is_empty() {
            println!("No releases found to sync.");
            return;
        }
        releases.sort();
        releases
    };

    let provider = crate::platforms::get_provider(&provider_name);

    for release_name in releases_to_sync {
        let path = release_path(&release_name);
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(release) = serde_json::from_str::<Release>(&content) {
                println!("\n📦 Syncing Release: {} (from seal {})", release_name, release.seal_id);

                // Step 1: Verify the seal exists locally
                let seal_path = format!(".dam/seals/{}.json", release.seal_id);
                if !Path::new(&seal_path).exists() {
                    println!("❌ Error: Seal '{}' for release '{}' not found locally.", release.seal_id, release_name);
                    continue;
                }

                // Step 2: Check if stream is up to date with remote
                println!("  ⏳ Checking if stream '{}' is up to date with remote...", release.stream);
                match provider.check_diff(&release.stream) {
                    Ok((_, behind)) => {
                        if behind > 0 {
                            println!("  ⚠️  Stream '{}' is {} commit(s) behind remote.", release.stream, behind);
                            if action.is_none() || action.as_ref().map_or(false, |a| a == "pull") {
                                println!("  📥 Pulling remote changes for stream '{}'...", release.stream);
                                if let Err(e) = provider.pull(&release.stream) {
                                    println!("  ❌ Failed to pull: {}. Skipping release sync.", e);
                                    continue;
                                }
                            }
                        } else if behind == 0 {
                            println!("  ✅ Stream '{}' is in sync with remote.", release.stream);
                        }
                    }
                    Err(e) => {
                        println!("  ❌ Failed to check diff: {}. Skipping release sync.", e);
                        continue;
                    }
                }

                // Step 3: Sync the release to the platform
                if action.is_some() && action.as_ref().map_or(false, |a| a == "pull") {
                    println!("  ℹ️  Release sync does not support pull action. Use push or omit action.");
                    continue;
                }

                println!("  📤 Syncing release '{}' to {}...", release_name, provider_name);
                match provider.sync_release(
                    &release.name,
                    &release.version,
                    &release.seal_id,
                    &release.stream,
                    release.description.as_ref(),
                    &release.tags,
                ) {
                    Ok(msg) => {
                        println!("  ✅ {}", msg);
                    }
                    Err(e) => {
                        println!("  ❌ Failed to sync release: {}", e);
                    }
                }
            }
        }
    }
}

