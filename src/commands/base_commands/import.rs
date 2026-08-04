use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use flate2::read::GzDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha2::Digest;
use tar::Archive;
use walkdir::WalkDir;
use zip::ZipArchive;
use crate::commands::base_commands::releases::Release;

pub fn run(source: String, merge: bool, branch: Option<String>) {
    let is_url = source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("git://")
        || source.starts_with("git@")
        || source.contains("github.com:")
        || source.contains("gitlab.com:")
        || source.contains("bitbucket.org:");

    if is_url {
        if source.ends_with(".dam") || source.ends_with(".seal") || source.ends_with(".zip") {
            match download_url_to_temp(&source) {
                Some(path) => {
                    import_local_path(&path, merge);
                    let _ = fs::remove_file(&path);
                }
                None => println!("Error: Failed to download '{}'.", source),
            }
            return;
        }

        match clone_git_url(&source, branch.as_deref()) {
            Some(clone_path) => {
                handle_cloned_repo(&clone_path, merge);
                if merge {
                    let _ = fs::remove_dir_all(&clone_path);
                }
            }
            None => println!("Error: Could not clone repository '{}'. Ensure git is installed and the URL is correct.", source),
        }
        return;
    }

    import_local_path(&source, merge);
}

fn import_local_path(source: &str, merge: bool) {
    let path = Path::new(source);
    if !path.exists() {
        println!("Error: Cannot locate path '{}'.", source);
        return;
    }

    if source.ends_with(".dam") {
        crate::core::project::import_project(source);
        return;
    }

    if source.ends_with(".seal") || source.ends_with(".zip") {
        let temp_dir = Path::new(".dam/tmp_import");
        fs::create_dir_all(temp_dir).unwrap();

        if source.ends_with(".seal") {
            let tar_gz = File::open(path).unwrap();
            let tar = GzDecoder::new(tar_gz);
            let mut archive = Archive::new(tar);
            archive.unpack(temp_dir).unwrap();
            integrate_from_temp(temp_dir, false);
        } else {
            let file = File::open(path).unwrap();
            let mut archive = ZipArchive::new(file).unwrap();
            archive.extract(temp_dir).unwrap();
            integrate_from_temp(temp_dir, true);
        }

        fs::remove_dir_all(temp_dir).unwrap();
        println!("Successfully imported from '{}'.", source);
        return;
    }

    if path.is_dir() {
        if merge && Path::new(".dam").exists() {
            if prompt_yes("Merge this directory into the current project? (Y/n): ") {
                copy_directory_contents(path, Path::new("."));
                if prompt_yes("Convert merged files into the current DAM reservoir now? (Y/n): ") {
                    create_or_update_dam_repo(Path::new("."), true, Some("main"));
                }
                println!("Merged repository contents into current project.");
                return;
            }
        }

        if prompt_yes(&format!("Convert the directory at '{}' into a DAM reservoir? (Y/n): ", source)) {
            create_or_update_dam_repo(path, false, Some("main"));
            println!("Converted directory to DAM at {}.", source);
        } else {
            println!("Directory is available at {}. No DAM conversion performed.", source);
        }
        return;
    }

    println!("Unsupported import source. Use a directory, .dam, .seal, or .zip path.");
}

fn download_url_to_temp(url: &str) -> Option<String> {
    let response = match reqwest::blocking::get(url) {
        Ok(r) => r,
        Err(e) => {
            println!("Download request failed: {}", e);
            return None;
        }
    };

    if !response.status().is_success() {
        println!("Download failed with status: {}", response.status());
        return None;
    }

    let file_name = url
        .split('/')
        .last()
        .filter(|v| !v.is_empty())
        .unwrap_or("downloaded.bin");
    let temp_path = std::env::temp_dir().join(file_name);

    let mut file = match File::create(&temp_path) {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to create temp file: {}", e);
            return None;
        }
    };

    let content = match response.bytes() {
        Ok(b) => b,
        Err(e) => {
            println!("Failed to read response body: {}", e);
            return None;
        }
    };

    if io::copy(&mut content.as_ref(), &mut file).is_err() {
        println!("Failed to write downloaded content to temp file.");
        return None;
    }

    temp_path.to_str().map(|s| s.to_string())
}

fn git_repo_name_from_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let last_segment = if let Some(pos) = trimmed.rfind('/') {
        &trimmed[pos + 1..]
    } else if let Some(pos) = trimmed.rfind(':') {
        &trimmed[pos + 1..]
    } else {
        trimmed
    };
    let name = last_segment.trim_end_matches(".git");
    if name.is_empty() {
        "imported-repo".to_string()
    } else {
        name.to_string()
    }
}

fn unique_clone_target(repo_name: &str) -> PathBuf {
    let mut target = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    target.push(repo_name);
    let mut suffix = 1;
    while target.exists() {
        target = env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(format!("{}_import_{}", repo_name, suffix));
        suffix += 1;
    }
    target
}

fn is_git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn clone_git_url(url: &str, branch: Option<&str>) -> Option<PathBuf> {
    if !is_git_available() {
        println!("Git is not available in your PATH. Git-backed import is unsupported in this environment.");
        println!("Please use a `.dam`, `.seal`, or `.zip` archive instead, or install Git and retry.");
        return None;
    }

    let repo_name = git_repo_name_from_url(url);
    let clone_path = unique_clone_target(&repo_name);

    let mut cmd = Command::new("git");
    cmd.arg("clone").arg("--no-single-branch");
    if let Some(br) = branch {
        cmd.arg("--branch").arg(br);
    }
    cmd.arg(url).arg(&clone_path);

    println!("Cloning {} into {}...", url, clone_path.display());
    let output = cmd.output();
    match output {
        Ok(output) if output.status.success() => Some(clone_path),
        Ok(output) => {
            println!("Git clone failed: {}", String::from_utf8_lossy(&output.stderr));
            None
        }
        Err(e) => {
            println!("Failed to execute git: {}", e);
            None
        }
    }
}

fn handle_cloned_repo(clone_path: &Path, merge: bool) {
    let repo_dir = clone_path;
    if merge && Path::new(".dam").exists() {
        if prompt_yes("Merge cloned repository into current project? (Y/n): ") {
            copy_directory_contents(repo_dir, Path::new("."));
            if prompt_yes("Convert merged files into the current DAM reservoir now? (Y/n): ") {
                create_or_update_dam_repo(Path::new("."), true, Some("main"));
            }
            return;
        }
    }

    if prompt_yes(&format!("Convert cloned repository at '{}' into a DAM reservoir? (Y/n): ", repo_dir.display())) {
        convert_git_repo_to_dam(repo_dir);
        println!("Converted cloned repository to DAM at {}.", repo_dir.display());
    } else {
        println!("Cloned repository is available at {}.", repo_dir.display());
    }
}

fn convert_git_repo_to_dam(repo_dir: &Path) {
    let mut branches = Vec::new();
    let mut tags = Vec::new();

    let branch_output = Command::new("git")
        .current_dir(repo_dir)
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads", "refs/remotes/origin"])
        .output()
        .ok();

    if let Some(output) = branch_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let branch = line.trim();
                if branch.is_empty() || branch == "HEAD" || branch.starts_with("origin/HEAD") {
                    continue;
                }
                let clean = branch.trim_start_matches("origin/");
                if !branches.contains(&clean.to_string()) {
                    branches.push(clean.to_string());
                }
            }
        }
    }

    let tag_output = Command::new("git")
        .current_dir(repo_dir)
        .args(["tag", "--list"])
        .output()
        .ok();
    if let Some(output) = tag_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let tag = line.trim();
                if !tag.is_empty() {
                    tags.push(tag.to_string());
                }
            }
        }
    }

    if branches.is_empty() {
        branches.push("main".to_string());
    }

    let default_branch = branches
        .iter()
        .find(|b| b.as_str() == "main" || b.as_str() == "master")
        .cloned()
        .unwrap_or_else(|| branches[0].clone());

    let mut created_streams = Vec::new();
    for (idx, branch) in branches.iter().enumerate() {
        let branch_name = branch.trim();
        let _ = Command::new("git")
            .current_dir(repo_dir)
            .args(["checkout", "-q", branch_name])
            .status();

        let stream_name = branch_name.trim_start_matches("origin/");
        let existing = idx > 0;
        create_or_update_dam_repo(repo_dir, existing, Some(stream_name));
        created_streams.push(stream_name.to_string());
    }

    let _ = Command::new("git")
        .current_dir(repo_dir)
        .args(["checkout", "-q", &default_branch])
        .status();

    let current_stream = if created_streams.contains(&default_branch) {
        default_branch.clone()
    } else {
        created_streams.first().cloned().unwrap_or_else(|| "main".to_string())
    };

    fs::write(repo_dir.join(".dam/CURRENT"), &current_stream).unwrap();
    // Write stream metadata files into the imported repository's .dam/streams directory
    let _ = fs::create_dir_all(repo_dir.join(".dam/streams"));
    for stream_name in &created_streams {
        let meta_path = repo_dir.join(format!(".dam/streams/{}", stream_name));

        // Try to load existing meta from the imported repo, otherwise build a new one
        let mut meta = if let Ok(content) = fs::read_to_string(&meta_path) {
            serde_json::from_str::<crate::commands::stream::StreamMeta>(&content).unwrap_or_default()
        } else {
            crate::commands::stream::StreamMeta::default()
        };

        meta.name = stream_name.to_string();
        meta.target = Some("main".to_string());
        meta.description = Some(format!("Imported Git branch '{}' as DAM stream", stream_name));
        meta.status = "Active".to_string();
        meta.owner = std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string());
        meta.created_at = chrono::Utc::now().to_rfc3339();

        fs::write(meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    }

    // Read active stream meta from the imported repo, not the caller CWD
    let active_meta_path = repo_dir.join(format!(".dam/streams/{}", current_stream));
    let default_seal = if let Ok(content) = fs::read_to_string(active_meta_path) {
        if let Ok(meta) = serde_json::from_str::<crate::commands::stream::StreamMeta>(&content) {
            meta.latest_seal.clone()
        } else { None }
    } else { None };

    if !tags.is_empty() {
        fs::create_dir_all(repo_dir.join(".dam/releases")).unwrap_or_default();
    }

    for tag in &tags {
        let release = Release {
            name: tag.clone(),
            version: tag.clone(),
            description: Some(format!("Imported Git tag '{}' from repository import", tag)),
            tags: vec!["git-tag".to_string()],
            created_at: chrono::Utc::now().to_rfc3339(),
            seal_id: default_seal.clone().unwrap_or_else(|| "seal_import_tag".to_string()),
            stream: current_stream.clone(),
        };

        let release_path = repo_dir.join(".dam/releases").join(format!("{}.json", tag));
        fs::write(release_path, serde_json::to_string_pretty(&release).unwrap()).unwrap();
    }

    let _ = Command::new("git")
        .current_dir(repo_dir)
        .args(["checkout", "-q", &default_branch])
        .status();
}

fn prompt_yes(question: &str) -> bool {
    print!("{}", question);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let trimmed = input.trim().to_lowercase();
    trimmed.is_empty() || trimmed == "y" || trimmed == "yes"
}

fn copy_directory_contents(src: &Path, dst: &Path) {
    for entry in WalkDir::new(src) {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path.strip_prefix(src).unwrap();

        if rel.as_os_str().is_empty() {
            continue;
        }
        if rel.starts_with(".git") || rel.starts_with(".dam") {
            continue;
        }

        let dest_path = dst.join(rel);
        if entry.path().is_dir() {
            let _ = fs::create_dir_all(&dest_path);
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let _ = fs::copy(path, &dest_path);
    }
}

fn create_or_update_dam_repo(target: &Path, existing: bool, stream_name: Option<&str>) {
    let stream_label = stream_name.unwrap_or("main");

    if !target.join(".dam").exists() {
        fs::create_dir_all(target.join(".dam/seals")).unwrap();
        fs::create_dir_all(target.join(".dam/objects")).unwrap();
        fs::create_dir_all(target.join(".dam/streams")).unwrap();
        fs::write(target.join(".dam/CURRENT"), stream_label).unwrap();

        let config = format!(r#"[reservoir]
version = "{}"
type = "native"
name = "{}"
suppress_nested_warning = false
purities_overrides_impurities = false
impurities_overrides_purities = false
enforce_password_on_project_import = false
disable_overwrite_check = false
overwrite_check_exclude = []
"#, env!("CARGO_PKG_VERSION"), target.file_name().and_then(|n| n.to_str()).unwrap_or("imported-project"));
        fs::write(target.join(".dam/config.toml"), config).unwrap();
    }

    let mut files_meta = Vec::new();
    let mut id_hasher = sha2::Sha256::new();
    for entry in WalkDir::new(target) {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path.strip_prefix(target).unwrap();

        if rel.as_os_str().is_empty() {
            continue;
        }
        if rel.starts_with(".dam") || rel.starts_with(".git") {
            continue;
        }

        if path.is_dir() {
            continue;
        }

        let mut file = File::open(path).unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();

        let mut file_hasher = sha2::Sha256::new();
        file_hasher.update(&buffer);
        let hash = format!("{:x}", file_hasher.finalize());

        let dest_obj = target.join(".dam/objects").join(&hash);
        if !dest_obj.exists() {
            let compressed_file = File::create(&dest_obj).unwrap();
            let mut encoder = ZlibEncoder::new(compressed_file, Compression::default());
            encoder.write_all(&buffer).unwrap();
            encoder.finish().unwrap();
        }

        let path_str = rel.to_string_lossy().replace("\\", "/");
        id_hasher.update(path_str.as_bytes());
        id_hasher.update(hash.as_bytes());

        files_meta.push(crate::commands::base_commands::seal::FileEntry {
            path: path_str,
            hash,
            is_dir: false,
        });
    }

    let id_hash = format!("{:x}", id_hasher.finalize());
    let seal_id = format!("seal_import_{}_{}", stream_label, &id_hash[..8]);
    let message = if existing {
        format!("Imported repository branch '{}' into existing DAM reservoir.", stream_label)
    } else {
        format!("Converted repository branch '{}' into a new DAM reservoir.", stream_label)
    };
    let timestamp = chrono::Utc::now().to_rfc3339();

    let parents = if existing {
        let meta = crate::commands::stream::get_or_create_meta(stream_label);
        if let Some(ref latest) = meta.latest_seal {
            vec![latest.clone()]
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let seal = crate::commands::base_commands::seal::Seal {
        id: seal_id.clone(),
        message,
        timestamp,
        files: files_meta,
        stream: stream_label.to_string(),
        parents,
    };

    fs::write(target.join(format!(".dam/seals/{}.json", seal_id)), serde_json::to_string_pretty(&seal).unwrap()).unwrap();
    fs::write(target.join(format!(".dam/streams/{}", stream_label)), "").unwrap();
    fs::write(target.join(".dam/staging.json"), "[]").unwrap();

    // Write stream metadata into the target repository rather than the current CWD.
    // Construct a StreamMeta and write it to <target>/.dam/streams/<stream_label>
    let stream_meta = crate::commands::stream::StreamMeta {
        name: stream_label.to_string(),
        description: Some(format!("Imported Git branch '{}' as DAM stream", stream_label)),
        owner: std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string()),
        priority: if stream_label == "main" { "High".to_string() } else { "Normal".to_string() },
        status: "Active".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        target: Some("main".to_string()),
        goals: vec![],
        notes: None,
        latest_seal: Some(seal_id.clone()),
    };

    let meta_path = target.join(format!(".dam/streams/{}", stream_label));
    fs::write(meta_path, serde_json::to_string_pretty(&stream_meta).unwrap()).unwrap();

    println!("Created DAM seal {} for imported repository branch '{}'.", seal_id, stream_label);
}

fn integrate_from_temp(temp_dir: &Path, compress_objects: bool) {
    for entry in fs::read_dir(temp_dir).unwrap().flatten() {
        let name = entry.file_name().into_string().unwrap();

        if name == "streams" {
            let _ = fs::create_dir_all(".dam/streams");
            for s in fs::read_dir(entry.path()).unwrap().flatten() {
                let dest = Path::new(".dam/streams").join(s.file_name());
                let _ = fs::copy(s.path(), dest);
            }
            continue;
        }

        if name == "releases" {
            let _ = fs::create_dir_all(".dam/releases");
            for r in fs::read_dir(entry.path()).unwrap().flatten() {
                let dest = Path::new(".dam/releases").join(r.file_name());
                let _ = fs::copy(r.path(), dest);
            }
            continue;
        }

        if name.ends_with(".json") {
            // Try to determine whether this JSON is a Seal or a Release
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if serde_json::from_str::<crate::commands::base_commands::seal::Seal>(&content).is_ok() {
                    let _ = fs::create_dir_all(".dam/seals");
                    let _ = fs::copy(entry.path(), Path::new(".dam/seals").join(&name));
                    continue;
                }
                if serde_json::from_str::<crate::commands::base_commands::releases::Release>(&content).is_ok() {
                    let _ = fs::create_dir_all(".dam/releases");
                    let _ = fs::copy(entry.path(), Path::new(".dam/releases").join(&name));
                    continue;
                }
            }
            // Fallback: copy into seals
            let _ = fs::create_dir_all(".dam/seals");
            let _ = fs::copy(entry.path(), Path::new(".dam/seals").join(&name));
        } else if name == "objects" && !compress_objects {
            for obj in fs::read_dir(entry.path()).unwrap().flatten() {
                let dest = Path::new(".dam/objects").join(obj.file_name());
                if !dest.exists() {
                    fs::copy(obj.path(), dest).unwrap();
                }
            }
        }
    }

    if compress_objects {
        if let Some(json_file) = fs::read_dir(temp_dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().ends_with(".json")) {
            let meta_content = fs::read_to_string(json_file.path()).unwrap();
            if let Ok(seal) = serde_json::from_str::<crate::commands::base_commands::seal::Seal>(&meta_content) {
                for file_meta in seal.files {
                    if !file_meta.is_dir {
                        let extracted_raw_file = temp_dir.join(&file_meta.path);
                        if extracted_raw_file.exists() {
                            let dest_obj = Path::new(".dam/objects").join(&file_meta.hash);
                            if !dest_obj.exists() {
                                let mut raw = File::open(extracted_raw_file).unwrap();
                                let compressed_file = File::create(dest_obj).unwrap();
                                let mut encoder = ZlibEncoder::new(compressed_file, Compression::default());
                                io::copy(&mut raw, &mut encoder).unwrap();
                                encoder.finish().unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}