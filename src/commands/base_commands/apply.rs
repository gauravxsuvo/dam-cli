use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::collections::HashMap;
use std::time::UNIX_EPOCH;
use serde_json;
use serde::{Deserialize, Serialize};
use crate::commands::seal;
use crate::commands::stream;
use crate::commands::base_commands::settings::get_toml_val;
use chrono::{DateTime, FixedOffset};
use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct HashCacheEntry {
    hash: String,
    mtime: i64,
    size: u64,
}

fn cache_path() -> &'static str {
    ".dam/hash_cache.json"
}

fn load_hash_cache() -> HashMap<String, HashCacheEntry> {
    if let Ok(s) = fs::read_to_string(cache_path()) {
        if let Ok(map) = serde_json::from_str(&s) {
            return map;
        }
    }
    HashMap::new()
}

fn save_hash_cache(map: &HashMap<String, HashCacheEntry>) {
    if let Ok(s) = serde_json::to_string_pretty(map) {
        let _ = fs::create_dir_all(".dam");
        let _ = fs::write(cache_path(), s);
    }
}

fn hash_file_with_cache(path: &Path, cache: &mut HashMap<String, HashCacheEntry>) -> Option<String> {
    let meta = path.metadata().ok()?;
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let size = meta.len();
    let key = path.to_string_lossy().to_string();

    if let Some(entry) = cache.get(&key) {
        if entry.mtime == mtime && entry.size == size {
            return Some(entry.hash.clone());
        }
    }

    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).ok()?;
    let computed = format!("{:x}", hasher.finalize());

    cache.insert(key, HashCacheEntry { hash: computed.clone(), mtime, size });
    Some(computed)
}

fn find_latest_global_seal() -> Option<String> {
    let seals_dir = Path::new(".dam/seals");
    if !seals_dir.exists() {
        return None;
    }

    let mut latest_id = None;
    let mut latest_ts: Option<DateTime<FixedOffset>> = None;

    for entry in fs::read_dir(seals_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(seal) = serde_json::from_str::<seal::Seal>(&content) {
                    if let Ok(ts) = DateTime::parse_from_rfc3339(&seal.timestamp) {
                        if latest_ts.as_ref().map_or(true, |current| ts > *current) {
                            latest_ts = Some(ts);
                            latest_id = Some(seal.id.clone());
                        }
                    } else if latest_id.is_none() {
                        latest_id = Some(seal.id.clone());
                    }
                }
            }
        }
    }

    latest_id
}

pub fn run_latest(preview: bool) {
    let current_stream = fs::read_to_string(".dam/CURRENT").unwrap_or_else(|_| "main".to_string()).trim().to_string();
    let stream_meta = stream::get_or_create_meta(&current_stream);

    if let Some(latest) = stream_meta.latest_seal {
        run(latest, preview);
    } else {
        println!("Error: No latest seal found in the current stream '{}'.", current_stream);
    }
}

pub fn run_latest_global(preview: bool) {
    if let Some(latest) = find_latest_global_seal() {
        run(latest, preview);
    } else {
        println!("Error: No seals found in the repository.");
    }
}

pub fn run(seal_id: String, preview: bool) {
    let seal_meta_path = format!(".dam/seals/{}.json", seal_id);
    if !Path::new(&seal_meta_path).exists() {
        println!("Error: Seal '{}' not found.", seal_id);
        return;
    }

    let meta_content = fs::read_to_string(&seal_meta_path).unwrap();
    let seal: seal::Seal = serde_json::from_str(&meta_content).unwrap();

    if preview {
        println!("\n--- Preview of Restoration [{}] ---", seal_id);
        println!("Description: {}", seal.message);
        for file in &seal.files {
            println!("  →  {}", file.path);
        }
        return;
    }

    // Safety Intercept: Verify if workspace tracking has items collected
    let staging_content = fs::read_to_string(".dam/staging.json").unwrap_or_else(|_| "[]".to_string());
    let staged_files: Vec<String> = serde_json::from_str(&staging_content).unwrap_or_default();

    if !staged_files.is_empty() {
        println!("\nWARNING: Unsaved changes detected in your staging area.");
        println!("Restoring may permanently overwrite adjustments made to those files.");
        print!("Would you like to auto-seal your current work before restoring? (Y/n): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input == "y" || input.is_empty() {
            print!("Enter safety seal message: ");
            io::stdout().flush().unwrap();
            let mut msg = String::new();
            io::stdin().read_line(&mut msg).unwrap();
            let msg = msg.trim();
            let fallback_msg = format!("Pre-apply snapshot for {}", seal_id);
            
            let final_msg = if msg.is_empty() { &fallback_msg } else { msg };
            seal::run(final_msg.to_string(),Vec::new());
        }
    }

    let config_content = fs::read_to_string(".dam/config.toml").unwrap_or_default();
    let overwrite_check_disabled = get_toml_val(&config_content, "disable_overwrite_check")
        .unwrap_or_else(|| "false".to_string())
        == "true";
    let overwrite_check_exclude: Vec<String> = get_toml_val(&config_content, "overwrite_check_exclude")
        .map(|v| {
            v.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Load hash cache to speed up repeated hash computations on large repos
    let mut hash_cache = load_hash_cache();

    if !overwrite_check_disabled {
        let mut changed_locally = Vec::new();
        for entry in &seal.files {
            if entry.is_dir || overwrite_check_exclude.contains(&entry.path) {
                continue;
            }
            let workspace_target = Path::new(&entry.path);
            if workspace_target.exists() {
                if let Some(current_hash) = hash_file_with_cache(workspace_target, &mut hash_cache) {
                    if current_hash != entry.hash {
                        changed_locally.push(entry.path.clone());
                    }
                }
            }
        }

        if !changed_locally.is_empty() {
            println!(
                "\n⚠️  WARNING: The following files have local changes that don't match seal {}:",
                seal_id
            );
            for path in &changed_locally {
                println!("  - {}", path);
            }
            print!("Overwrite these files anyway? (y/N): ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted. No files were changed.");
                // Save partial cache updates so subsequent runs benefit
                save_hash_cache(&hash_cache);
                return;
            }
        }
    }

    println!("Restoring workspace files to matches from {}...", seal_id);

    for entry in &seal.files {
        let workspace_target = Path::new(&entry.path);

        if entry.is_dir {
            fs::create_dir_all(workspace_target).unwrap();
            println!("  ✓ Applied Directory: {}", entry.path);
        } else {
            if let Some(parent) = workspace_target.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            
            let obj_path = Path::new(".dam/objects").join(&entry.hash);
            if obj_path.exists() {
                let compressed_file = File::open(obj_path).unwrap();
                let mut decoder = ZlibDecoder::new(compressed_file);
                let mut out_file = File::create(workspace_target).unwrap();
                io::copy(&mut decoder, &mut out_file).unwrap();
                println!("  ✓ Applied: {}", entry.path);
            } else {
                println!("  ! Warning: Object {} missing for file {}", entry.hash, entry.path);
            }
        }
    }
    // Save updated hash cache for future runs
    save_hash_cache(&hash_cache);

    println!("\nStream successfully brought back to state: {}", seal_id);
}