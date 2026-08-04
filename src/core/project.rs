use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use zip::write::FileOptions;
use glob::Pattern;

use crate::core::crypto;
use crate::providers;

#[derive(Serialize, Deserialize, Debug)]
pub struct ProjectMetadata {
    pub project_name: String,
    pub author: String,
    pub timestamp: String,
    pub provider: String,
    pub profile_used: String,
    pub setup_commands: Vec<String>,
    pub is_encrypted: bool,
    pub streams: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct DamToml {
    project: ProjectConfig,
    setup: SetupConfig,
    profiles: std::collections::HashMap<String, ProfileConfig>,
}

#[derive(Deserialize, Debug)]
struct ProjectConfig {
    name: String,
    provider: String,
}

#[derive(Deserialize, Debug)]
struct SetupConfig {
    commands: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct ProfileConfig {
    description: String,
    include: Vec<String>,
    exclude: Vec<String>,
    streams: Option<Vec<String>>,
}

pub fn export_project(project_name: &str, profile_name: Option<String>) {
    let toml_content = fs::read_to_string("dam.toml").expect("dam.toml not found! Run 'dam source' to initialize.");
    let config: DamToml = toml::from_str(&toml_content).unwrap();

    let config_toml_content = fs::read_to_string(".dam/config.toml").unwrap_or_default();
    let enforce_pwd = config_toml_content.lines().any(|l| l.trim() == "enforce_password_on_project_import = true");

    let target_profile = profile_name.unwrap_or_else(|| "full".to_string());
    let profile = config.profiles.get(&target_profile).expect("Profile not found in dam.toml");

    let mut password = None;
    
    if enforce_pwd {
        password = Some(rpassword::prompt_password("Project requires encryption. Enter password for .dam archive: ").unwrap());
    }

    println!("Packing project '{}' using profile '{}' ({})", config.project.name, target_profile, profile.description);

    // 1. Build Tar.Gz Payload in memory
    let mut tar_gz_buffer = Vec::new();
    {
        let enc = GzEncoder::new(&mut tar_gz_buffer, Compression::best());
        let mut tar = tar::Builder::new(enc);

        for entry in WalkDir::new(".") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let clean_path = path.strip_prefix(".").unwrap().to_str().unwrap().replace("\\", "/");
                
                if is_allowed(&clean_path, &profile.include, &profile.exclude) {
                    tar.append_path_with_name(path, &clean_path).unwrap();
                }
            }
        }
        // Always include important DAM metadata directories if present
        if Path::new(".dam/releases").exists() {
            for entry in fs::read_dir(".dam/releases").unwrap().flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = format!(".dam/releases/{}", entry.file_name().to_string_lossy());
                    tar.append_path_with_name(&p, &name).unwrap();
                }
            }
        }
        if Path::new(".dam/streams").exists() {
            for entry in fs::read_dir(".dam/streams").unwrap().flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = format!(".dam/streams/{}", entry.file_name().to_string_lossy());
                    tar.append_path_with_name(&p, &name).unwrap();
                }
            }
        }
        tar.finish().unwrap();
    }

    // 2. Encrypt if necessary
    let payload = if let Some(pwd) = &password {
        crypto::encrypt(&tar_gz_buffer, pwd).expect("Encryption failed")
    } else {
        tar_gz_buffer
    };

    // 3. Create Metadata
    let meta = ProjectMetadata {
        project_name: config.project.name.clone(),
        author: std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string()),
        timestamp: Utc::now().to_rfc3339(),
        provider: config.project.provider.clone(),
        profile_used: target_profile.clone(),
        setup_commands: config.setup.commands.clone(),
        is_encrypted: enforce_pwd,
        streams: profile.streams.clone().or_else(get_export_streams),
    };
    let meta_json = serde_json::to_string_pretty(&meta).unwrap();

    // 4. Wrap everything in a Zip (.dam) file
    let output_filename = format!("{}.dam", project_name);
    let file = File::create(&output_filename).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("metadata.json", options).unwrap();
    zip.write_all(meta_json.as_bytes()).unwrap();

    let payload_name = if enforce_pwd { "payload.enc" } else { "payload.tar.gz" };
    zip.start_file(payload_name, options).unwrap();
    zip.write_all(&payload).unwrap();

    zip.finish().unwrap();
    println!("Export successful! Created highly compressed archive: {}", output_filename);
}

pub fn import_project(file_path: &str) {
    let file = File::open(file_path).expect("Archive not found");
    let mut archive = zip::ZipArchive::new(file).unwrap();

    // 1. Read Metadata
    let mut meta_file = archive.by_name("metadata.json").expect("Invalid .dam archive: missing metadata.json");
    let mut meta_json = String::new();
    meta_file.read_to_string(&mut meta_json).unwrap();
    let meta: ProjectMetadata = serde_json::from_str(&meta_json).unwrap();
    drop(meta_file);

    println!("Importing Project: {}", meta.project_name);
    println!("Provider: {} | Profile: {}", meta.provider, meta.profile_used);

    // 2. Provider Check
    let provider = providers::get_provider(&meta.provider);
    if !provider.check_environment() {
        println!("Warning: The environment for '{}' does not seem properly configured on this machine.", meta.provider);
        let resp = rpassword::prompt_password("Continue anyway? (y/N): ").unwrap();
        if !resp.eq_ignore_ascii_case("y") { return; }
    }

    // 3. Resolve Payload
    let mut payload = Vec::new();
    if meta.is_encrypted {
        let mut enc_file = archive.by_name("payload.enc").unwrap();
        let mut enc_data = Vec::new();
        enc_file.read_to_end(&mut enc_data).unwrap();
        
        let pwd = rpassword::prompt_password("Archive is encrypted. Enter password: ").unwrap();
        payload = crypto::decrypt(&enc_data, &pwd).expect("Decryption failed! Incorrect password.");
    } else {
        let mut tar_file = archive.by_name("payload.tar.gz").unwrap();
        tar_file.read_to_end(&mut payload).unwrap();
    }

    // 4. Merge or New Folder
    let mut target_dir = std::path::PathBuf::from(&meta.project_name);
    
    if Path::new("dam.toml").exists() {
        println!("A project already exists in this directory.");
        let choice = rpassword::prompt_password("Merge into current directory (m) or create new folder (n)? [m/N]: ").unwrap();
        if choice.eq_ignore_ascii_case("m") {
            target_dir = std::path::PathBuf::from(".");
        } else {
            fs::create_dir_all(&target_dir).unwrap();
        }
    } else {
        // Safe default: extract to the <proj_name> directory
        fs::create_dir_all(&target_dir).unwrap();
    }

    // 5. Extract Tar
    let cursor = Cursor::new(payload);
    let tar = GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(&target_dir).unwrap();
    println!("Successfully extracted files to {}", target_dir.display());

    // 6. Handle Setup Commands
    // 6. Handle Setup Commands with trusted/untrusted policy
    if !meta.setup_commands.is_empty() {
        // Determine provider-allowed commands
        let allowed = providers::allowed_commands_for(&meta.provider);

        let mut trusted: Vec<String> = Vec::new();
        let mut untrusted: Vec<String> = Vec::new();

        for cmd in &meta.setup_commands {
            let mut matched = false;
            for a in &allowed {
                if cmd.trim().starts_with(a) {
                    matched = true;
                    break;
                }
            }
            if matched {
                trusted.push(cmd.clone());
            } else {
                untrusted.push(cmd.clone());
            }
        }

        println!("\nSetup Commands (provider: {})", meta.provider);
        println!("─────────────────────────────────────");
        for t in &trusted {
            println!("  ✅ Trusted : {}", t);
        }
        for u in &untrusted {
            println!("  ⚠️  Untrusted: {}", u);
        }

        // Single block: ask to run trusted commands
        if !trusted.is_empty() {
            println!("\nTrusted commands can be executed now.");
            print!("Execute trusted commands now? (Y/n): ");
            std::io::stdout().flush().unwrap();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            let choice = input.trim().to_lowercase();
            if choice.is_empty() || choice == "y" {
                for cmd in &trusted {
                    println!("Running: {}", cmd);
                    let status = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .current_dir(&target_dir)
                        .status();
                    match status {
                        Ok(s) if s.success() => println!("  ✓ Command succeeded"),
                        Ok(s) => println!("  ❌ Command failed: exit {}",&s),
                        Err(e) => println!("  ❌ Failed to run command: {}", e),
                    }
                }
            } else {
                println!("Skipping trusted command execution.");
            }
        }

        // Always write untrusted commands to setup_untrusted.sh for review
        if !untrusted.is_empty() {
            let setup_script = target_dir.join("setup_untrusted.sh");
            let script_content = format!("#!/bin/bash\n# UNTRUSTED: Review before executing\n\n{}", untrusted.join("\n"));
            fs::write(&setup_script, script_content).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&setup_script).unwrap().permissions();
                perms.set_mode(0o750);
                let _ = fs::set_permissions(&setup_script, perms);
            }
            println!("Untrusted commands written to {}. Review before running.", setup_script.display());

            // Require explicit 'yes' to run untrusted commands immediately
            print!("Type 'yes' to execute untrusted commands now, or press Enter to keep for review: ");
            std::io::stdout().flush().unwrap();
            let mut confirm = String::new();
            std::io::stdin().read_line(&mut confirm).unwrap();
            if confirm.trim() == "yes" {
                for cmd in &untrusted {
                    println!("Running untrusted: {}", cmd);
                    let status = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .current_dir(&target_dir)
                        .status();
                    match status {
                        Ok(s) if s.success() => println!("  ✓ Command succeeded"),
                        Ok(s) => println!("  ❌ Command failed: exit {}", &s),
                        Err(e) => println!("  ❌ Failed to run command: {}", e),
                    }
                }
            } else {
                println!("Left untrusted commands in {} for manual review.", setup_script.display());
            }
        }
    }
}

pub fn inspect_project(file_path: &str) {
    let file = File::open(file_path).expect("Archive not found");
    let mut archive = zip::ZipArchive::new(file).unwrap();

    let mut meta_file = archive.by_name("metadata.json").expect("Invalid .dam archive");
    let mut meta_json = String::new();
    meta_file.read_to_string(&mut meta_json).unwrap();
    let meta: ProjectMetadata = serde_json::from_str(&meta_json).unwrap();

    println!("\n--- DAM PROJECT ARCHIVE INSPECTION ---");
    println!("Project Name : {}", meta.project_name);
    println!("Author       : {}", meta.author);
    println!("Timestamp    : {}", meta.timestamp);
    println!("Provider     : {}", meta.provider);
    println!("Profile Used : {}", meta.profile_used);
    println!("Encrypted    : {}", meta.is_encrypted);
    
    println!("\nSetup Commands:");
    for cmd in meta.setup_commands {
        println!("  > {}", cmd);
    }
    println!("--------------------------------------\n");
}

fn get_export_streams() -> Option<Vec<String>> {
    let streams_dir = Path::new(".dam/streams");
    if !streams_dir.exists() {
        return None;
    }
    let streams = fs::read_dir(streams_dir)
        .ok()?
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    if streams.is_empty() {
        None
    } else {
        Some(streams)
    }
}

fn is_allowed(path: &str, includes: &[String], excludes: &[String]) -> bool {
    let mut allowed = false;
    for inc in includes {
        if Pattern::new(inc).unwrap().matches(path) { allowed = true; break; }
    }
    if !allowed { return false; }
    
    for exc in excludes {
        if Pattern::new(exc).unwrap().matches(path) { return false; }
    }
    true
}