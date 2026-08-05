use std::fs;
use std::path::Path;

use crate::cli::{DamTomlCommands, RegenerateCommands};

fn load_toml() -> toml::Table {
    let path = Path::new("dam.toml");
    if !path.exists() {
        return toml::Table::new();
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    match content.parse::<toml::Value>() {
        Ok(v) => match v {
            toml::Value::Table(table) => table,
            _ => toml::Table::new(),
        },
        Err(_) => toml::Table::new(),
    }
}

fn write_toml(table: &toml::Table) {
    fs::write("dam.toml", table.to_string()).unwrap_or_else(|e| {
        eprintln!("Failed to write dam.toml: {}", e);
    });
}

fn ensure_project_entry(table: &mut toml::Table, project_name: &str) {
    let project = table
        .entry("project")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let project_table = project.as_table_mut().unwrap();
    project_table.insert(
        "name".to_string(),
        toml::Value::String(project_name.to_string()),
    );
    project_table
        .entry("description")
        .or_insert(toml::Value::String("Project managed by DAM".to_string()));
    project_table
        .entry("enforce_password_on_project_import")
        .or_insert(toml::Value::Boolean(false));
}

fn ensure_provider_block(table: &mut toml::Table) {
    let provider = table
        .entry("provider")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let provider_table = provider.as_table_mut().unwrap();
    let custom = provider_table
        .entry("custom")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let custom_table = custom.as_table_mut().unwrap();
    custom_table
        .entry("any")
        .or_insert(toml::Value::Boolean(true));
}

fn ensure_streams_block(table: &mut toml::Table) {
    table
        .entry("streams")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
}

fn ensure_releases_block(table: &mut toml::Table) {
    table
        .entry("releases")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
}

fn project_name() -> String {
    if let Ok(dir) = std::env::current_dir() {
        if let Some(name) = dir.file_name() {
            return name.to_string_lossy().into_owned();
        }
    }
    "dam-project".to_string()
}

fn stream_names() -> Vec<String> {
    let stream_dir = Path::new(".dam/streams");
    if !stream_dir.exists() {
        return Vec::new();
    }

    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(stream_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                names.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    names.sort();
    names
}

fn release_names() -> Vec<String> {
    let rel_dir = Path::new(".dam/releases");
    if !rel_dir.exists() {
        return Vec::new();
    }

    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(rel_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_stem() {
                    names.push(name.to_string_lossy().to_string());
                }
            }
        }
    }
    names.sort();
    names
}

fn stable_versions_for_stream(stream: &str) -> Vec<String> {
    let dir = Path::new(".dam/stable_versions");
    if !dir.exists() {
        return Vec::new();
    }

    let mut values = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(entry_stream) = json.get("stream").and_then(|v| v.as_str()) {
                        if entry_stream == stream {
                            values.push(name.clone());
                        }
                    }
                }
            }
        }
    }
    values.sort();
    values
}

fn populate_streams(value: &mut toml::Table, target: Option<&str>) {
    ensure_streams_block(value);
    let stream_table = value["streams"].as_table_mut().unwrap();

    for name in stream_names() {
        if let Some(stream_filter) = target {
            if name != stream_filter {
                continue;
            }
        }

        let stream_entry = stream_table
            .entry(name.clone())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let stream = stream_entry.as_table_mut().unwrap();
        stream
            .entry("target")
            .or_insert(toml::Value::String(name.clone()));
        stream
            .entry("status")
            .or_insert(toml::Value::String("active".to_string()));

        let stream_meta_path = Path::new(".dam/streams").join(&name);
        if let Ok(content) = fs::read_to_string(&stream_meta_path) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(target_name) = meta.get("target").and_then(|v| v.as_str()) {
                    stream.insert(
                        "target".to_string(),
                        toml::Value::String(target_name.to_string()),
                    );
                }
                if let Some(status) = meta.get("status").and_then(|v| v.as_str()) {
                    stream.insert(
                        "status".to_string(),
                        toml::Value::String(status.to_string()),
                    );
                }
                if let Some(description) = meta.get("description").and_then(|v| v.as_str()) {
                    stream.insert(
                        "description".to_string(),
                        toml::Value::String(description.to_string()),
                    );
                }
                if let Some(goals) = meta.get("goals").and_then(|v| v.as_array()) {
                    let values = goals
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| toml::Value::String(s.to_string())))
                        .collect::<Vec<_>>();
                    if !values.is_empty() {
                        stream.insert("goals".to_string(), toml::Value::Array(values));
                    }
                }
            }
        }

        let stable_versions = stable_versions_for_stream(&name);
        if !stable_versions.is_empty() {
            let values = stable_versions
                .into_iter()
                .map(toml::Value::String)
                .collect();
            stream.insert("stable".to_string(), toml::Value::Array(values));
        }
    }
}

fn populate_releases(value: &mut toml::Table, selected: Option<&[String]>) {
    ensure_releases_block(value);
    let releases = value["releases"].as_table_mut().unwrap();
    releases.clear();

    for release in release_names() {
        if let Some(selection) = selected {
            if !selection.iter().any(|r| r == &release) {
                continue;
            }
        }

        let release_path = Path::new(".dam/releases").join(format!("{}.json", release));
        if let Ok(content) = fs::read_to_string(&release_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let mut release_table = toml::Table::new();
                if let Some(seal_id) = json.get("seal_id").and_then(|v| v.as_str()) {
                    release_table
                        .insert("seal".to_string(), toml::Value::String(seal_id.to_string()));
                }
                if let Some(stream) = json.get("stream").and_then(|v| v.as_str()) {
                    release_table.insert(
                        "stream".to_string(),
                        toml::Value::String(stream.to_string()),
                    );
                }
                if let Some(description) = json.get("description").and_then(|v| v.as_str()) {
                    release_table.insert(
                        "description".to_string(),
                        toml::Value::String(description.to_string()),
                    );
                }
                if let Some(tags) = json.get("tags").and_then(|v| v.as_array()) {
                    let values = tags
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| toml::Value::String(s.to_string())))
                        .collect::<Vec<_>>();
                    if !values.is_empty() {
                        release_table.insert("tags".to_string(), toml::Value::Array(values));
                    }
                }
                releases.insert(release.clone(), toml::Value::Table(release_table));
                continue;
            }
        }

        let mut fallback = toml::Table::new();
        fallback.insert("name".to_string(), toml::Value::String(release.clone()));
        releases.insert(release.clone(), toml::Value::Table(fallback));
    }
}

fn regenerate_from_disk(target: Option<&str>, force_toml: bool) {
    let mut table = load_toml();
    ensure_project_entry(&mut table, &project_name());
    ensure_provider_block(&mut table);
    if let Some(stream_name) = target {
        populate_streams(&mut table, Some(stream_name));
    } else {
        populate_streams(&mut table, None);
    }
    populate_releases(&mut table, None);

    if force_toml || !Path::new("dam.toml").exists() {
        write_toml(&table);
        println!("Regenerated dam.toml from live reservoir state.");
    } else {
        write_toml(&table);
        println!("Updated dam.toml from live reservoir state.");
    }
}

fn get_default_stream_selection() -> Vec<String> {
    let mut defaults = Vec::new();
    for name in stream_names() {
        if name == "main" {
            defaults.push(name);
            continue;
        }
        let meta_path = Path::new(".dam/streams").join(&name);
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(priority) = json.get("priority").and_then(|v| v.as_str()) {
                    if priority.eq_ignore_ascii_case("high") {
                        defaults.push(name);
                    }
                }
            }
        }
    }
    defaults.sort();
    defaults
}

fn add_profile_to_table(table: &mut toml::Table, name: &str, description: &str, includes: Vec<String>, excludes: Vec<String>) {
    let profiles = table.entry("profiles").or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let profiles_table = profiles.as_table_mut().unwrap();
    let mut profile = toml::Table::new();
    profile.insert("description".to_string(), toml::Value::String(description.to_string()));
    let inc_vals = includes.into_iter().map(|s| toml::Value::String(s)).collect::<Vec<_>>();
    let exc_vals = excludes.into_iter().map(|s| toml::Value::String(s)).collect::<Vec<_>>();
    profile.insert("include".to_string(), toml::Value::Array(inc_vals));
    profile.insert("exclude".to_string(), toml::Value::Array(exc_vals));
    profiles_table.insert(name.to_string(), toml::Value::Table(profile));
}

fn prompt_input(prompt: &str) -> String {
    print!("{}", prompt);
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn known_provider_names() -> Vec<&'static str> {
    vec!["flutter", "firebase", "python", "rust", "js", "custom"]
}

fn existing_provider_names() -> Vec<String> {
    let table = load_toml();
    table
        .get("provider")
        .and_then(|v| v.as_table())
        .map(|providers| providers.keys().cloned().collect())
        .unwrap_or_default()
}

fn provider_block_details(name: &str) {
    let table = load_toml();
    if let Some(provider) = table
        .get("provider")
        .and_then(|v| v.get(name))
        .and_then(|v| v.as_table())
    {
        println!("Provider '{}':", name);
        println!("  any = {}", provider.get("any").and_then(|v| v.as_bool()).unwrap_or(false));
        if let Some(sdk) = provider.get("sdk").and_then(|v| v.as_str()) {
            println!("  sdk = {}", sdk);
        }
        if let Some(cli) = provider.get("cli").and_then(|v| v.as_str()) {
            println!("  cli = {}", cli);
        }
        if let Some(cli_name) = provider.get("cli_name").and_then(|v| v.as_str()) {
            println!("  cli_name = {}", cli_name);
        }
        if let Some(version_cmd) = provider.get("version_check_command").and_then(|v| v.as_str()) {
            println!("  version_check_command = {}", version_cmd);
        }
        if let Some(setup) = provider.get("setup_commands").and_then(|v| v.as_array()) {
            let cmds: Vec<_> = setup.iter().filter_map(|v| v.as_str()).collect();
            if !cmds.is_empty() {
                println!("  setup_commands = {:?}", cmds);
            }
        }
    } else {
        println!("No provider block found for '{}'.", name);
    }
}

fn remove_provider_block(name: &str) {
    let mut table = load_toml();
    if let Some(provider_table) = table.get_mut("provider").and_then(|v| v.as_table_mut()) {
        if provider_table.remove(name).is_some() {
            write_toml(&table);
            println!("Removed provider block '{}'.", name);
            return;
        }
    }
    println!("No provider block named '{}' found.", name);
}


fn add_provider_full(
    name: &str,
    cli_name: Option<&str>,
    version_check_cmd: Option<&str>,
    setup_cmds: Vec<String>,
    sdk_req: Option<&str>,
    cli_req: Option<&str>,
) {
    let mut table = load_toml();
    ensure_project_entry(&mut table, &project_name());
    ensure_provider_block(&mut table);
    let providers = table["provider"].as_table_mut().unwrap();
    let entry = providers
        .entry(name.to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let provider_table = entry.as_table_mut().unwrap();

    provider_table.insert("any".to_string(), toml::Value::Boolean(false));
    provider_table.insert("verified".to_string(), toml::Value::Boolean(false));

    if let Some(cn) = cli_name {
        provider_table.insert("cli_name".to_string(), toml::Value::String(cn.to_string()));
    }
    if let Some(vcc) = version_check_cmd {
        provider_table.insert(
            "version_check_command".to_string(),
            toml::Value::String(vcc.to_string()),
        );
    }
    if !setup_cmds.is_empty() {
        let cmd_values: Vec<toml::Value> = setup_cmds
            .into_iter()
            .map(|c| toml::Value::String(c))
            .collect();
        provider_table.insert("setup_commands".to_string(), toml::Value::Array(cmd_values));
    }
    if let Some(sdk) = sdk_req {
        provider_table.insert("sdk".to_string(), toml::Value::String(sdk.to_string()));
    }
    if let Some(cli) = cli_req {
        provider_table.insert("cli".to_string(), toml::Value::String(cli.to_string()));
    }

    write_toml(&table);
    println!("Saved provider block '{}'.", name);
}

fn list_profiles() -> Vec<String> {
    let table = load_toml();
    table
        .get("profiles")
        .and_then(|v| v.as_table())
        .map(|profiles| profiles.keys().cloned().collect())
        .unwrap_or_default()
}

fn show_profile_details(name: &str) {
    let table = load_toml();
    if let Some(profile) = table
        .get("profiles")
        .and_then(|v| v.get(name))
        .and_then(|v| v.as_table())
    {
        println!("Profile '{}':", name);
        if let Some(description) = profile.get("description").and_then(|v| v.as_str()) {
            println!("  description = {}", description);
        }
        if let Some(include) = profile.get("include").and_then(|v| v.as_array()) {
            let values = include
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>();
            println!("  include = {:?}", values);
        }
        if let Some(exclude) = profile.get("exclude").and_then(|v| v.as_array()) {
            let values = exclude
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>();
            println!("  exclude = {:?}", values);
        }
    } else {
        println!("No profile named '{}' found.", name);
    }
}

fn remove_profile(name: &str) {
    let mut table = load_toml();
    if let Some(profiles) = table.get_mut("profiles").and_then(|v| v.as_table_mut()) {
        if profiles.remove(name).is_some() {
            write_toml(&table);
            println!("Removed profile '{}'.", name);
            return;
        }
    }
    println!("No profile named '{}' found.", name);
}

fn prompt_profile_details(
    current_description: Option<&str>,
    current_include: Option<&[String]>,
    current_exclude: Option<&[String]>,
) -> (String, Vec<String>, Vec<String>) {
    let default_description = current_description.unwrap_or("");
    let description = prompt_input(&format!(
        "Description [{}]: ",
        default_description
    ));
    let description = if description.is_empty() {
        default_description.to_string()
    } else {
        description
    };

    let default_include = current_include
        .map(|values| values.join(", "))
        .unwrap_or_else(|| "**/*".to_string());
    let include_input = prompt_input(&format!("Include patterns [{}]: ", default_include));
    let includes = if include_input.is_empty() {
        default_include
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        include_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let default_exclude = current_exclude
        .map(|values| values.join(", "))
        .unwrap_or_else(|| ".git/**, .dam/**".to_string());
    let exclude_input = prompt_input(&format!("Exclude patterns [{}]: ", default_exclude));
    let excludes = if exclude_input.is_empty() {
        default_exclude
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        exclude_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    (description, includes, excludes)
}

fn current_release_names_in_toml() -> Vec<String> {
    let table = load_toml();
    table
        .get("releases")
        .and_then(|v| v.as_table())
        .map(|releases| releases.keys().cloned().collect())
        .unwrap_or_default()
}

fn apply_release_selection(selection: Option<Vec<String>>) {
    let mut table = load_toml();
    ensure_project_entry(&mut table, &project_name());
    ensure_provider_block(&mut table);
    populate_releases(&mut table, selection.as_deref());
    write_toml(&table);
}

fn interactive_dam_toml_menu(stream: Option<&str>, force_toml: bool) {
    if let Some(stream_name) = stream {
        regenerate_from_disk(Some(stream_name), force_toml);
        return;
    }

    let available = stream_names();
    let defaults = get_default_stream_selection();
    println!("Available streams: {}", available.join(", "));
    println!("Default selection: {}", defaults.join(", "));
    println!("Press Enter to accept defaults, enter comma-separated streams, or type 'all' to include all.");
    let input = prompt_input("Streams to include: ");
    let selected: Vec<String> = if input.is_empty() {
        defaults.clone()
    } else if input.eq_ignore_ascii_case("all") {
        available.clone()
    } else {
        input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let mut table = load_toml();
    ensure_project_entry(&mut table, &project_name());
    ensure_provider_block(&mut table);
    for s in &selected {
        populate_streams(&mut table, Some(s));
    }

    let release_names = release_names();
    println!("Available releases: {}", release_names.join(", "));
    println!("Press Enter to include all available releases, or enter comma-separated release names to include.");
    let release_input = prompt_input("Releases to include: ");
    let selected_releases: Option<Vec<String>> = if release_input.is_empty() {
        None
    } else {
        Some(
            release_input
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    };
    populate_releases(&mut table, selected_releases.as_deref());

    write_toml(&table);
    println!("Wrote dam.toml with {} streams.", selected.len());
}

fn interactive_provider_menu() {
    let known = known_provider_names();
    println!("Recognized providers: {}", known.join(", "));
    println!("Note: You can add ANY provider (golang, ruby, custom, etc.)");
    let existing = existing_provider_names();
    if !existing.is_empty() {
        println!("Existing provider blocks: {}", existing.join(", "));
    }

    let pname = prompt_input("Provider name to manage: ");
    if pname.is_empty() {
        println!("No provider specified.");
        return;
    }

    if existing.contains(&pname) {
        provider_block_details(&pname);
        let action = prompt_input("[E]dit, [D]etails, [R]emove, or [C]ancel? ");
        match action.to_lowercase().as_str() {
            "e" | "d" => {
                println!("\n--- Edit Provider '{}' ---", pname);
                let cli_name = prompt_input("CLI command name (e.g., 'go', 'ruby'): ");
                let version_cmd = prompt_input("Version check command (e.g., '--version', 'version'): ");
                let setup_input = prompt_input("Setup commands (comma-separated, or leave blank): ");
                let setup_cmds: Vec<String> = if setup_input.is_empty() {
                    vec![]
                } else {
                    setup_input
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                };
                let sdk_req = prompt_input("SDK requirement (e.g., '>=1.20', optional): ");
                let cli_req = prompt_input("CLI requirement (e.g., '>=1.0', optional): ");

                add_provider_full(
                    &pname,
                    if cli_name.is_empty() { None } else { Some(cli_name.as_str()) },
                    if version_cmd.is_empty() { None } else { Some(version_cmd.as_str()) },
                    setup_cmds,
                    if sdk_req.is_empty() { None } else { Some(sdk_req.as_str()) },
                    if cli_req.is_empty() { None } else { Some(cli_req.as_str()) },
                );
                println!("Updated provider '{}'.", pname);
            }
            "r" => remove_provider_block(&pname),
            _ => println!("Canceled provider edit."),
        }
        return;
    }

    println!("\n--- Add New Provider '{}' ---", pname);
    println!("For unknown providers, specify how to detect and verify them.");
    let cli_name = prompt_input("CLI command name (e.g., 'go', 'ruby'): ");
    let version_cmd = prompt_input("Version check command (e.g., '--version', 'version'): ");
    let setup_input = prompt_input("Setup commands (comma-separated, or leave blank): ");
    let setup_cmds: Vec<String> = if setup_input.is_empty() {
        vec![]
    } else {
        setup_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let sdk_req = prompt_input("SDK requirement (e.g., '>=1.20', optional): ");
    let cli_req = prompt_input("CLI requirement (e.g., '>=1.0', optional): ");

    add_provider_full(
        &pname,
        if cli_name.is_empty() { None } else { Some(cli_name.as_str()) },
        if version_cmd.is_empty() { None } else { Some(version_cmd.as_str()) },
        setup_cmds,
        if sdk_req.is_empty() { None } else { Some(sdk_req.as_str()) },
        if cli_req.is_empty() { None } else { Some(cli_req.as_str()) },
    );
}

fn interactive_profile_menu() {
    let existing = list_profiles();
    if !existing.is_empty() {
        println!("Existing profiles: {}", existing.join(", "));
    } else {
        println!("No profiles exist in dam.toml yet.");
    }

    loop {
        let action = prompt_input("Choose [A]dd, [E]dit, [R]emove, or [Q]uit: ");
        match action.to_lowercase().as_str() {
            "a" => {
                let pname = prompt_input("Profile name: ");
                if pname.is_empty() {
                    println!("Profile name cannot be blank.");
                    continue;
                }
                let (description, includes, excludes) = prompt_profile_details(None, None, None);
                let mut table = load_toml();
                ensure_project_entry(&mut table, &project_name());
                add_profile_to_table(&mut table, &pname, &description, includes, excludes);
                write_toml(&table);
                println!("Added profile '{}'.", pname);
            }
            "e" => {
                let pname = prompt_input("Profile to edit: ");
                if pname.is_empty() {
                    continue;
                }
                if !existing.contains(&pname) {
                    println!("Profile '{}' not found.", pname);
                    continue;
                }
                show_profile_details(&pname);
                let table = load_toml();
                let profile = table
                    .get("profiles")
                    .and_then(|v| v.get(&pname))
                    .and_then(|v| v.as_table());
                let description = profile
                    .and_then(|p| p.get("description").and_then(|v| v.as_str()))
                    .unwrap_or("");
                let include = profile
                    .and_then(|p| p.get("include").and_then(|v| v.as_array()))
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>());
                let exclude = profile
                    .and_then(|p| p.get("exclude").and_then(|v| v.as_array()))
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<_>>());
                let (description, includes, excludes) = prompt_profile_details(
                    Some(description),
                    include.as_deref(),
                    exclude.as_deref(),
                );
                let mut table = load_toml();
                if let Some(profiles) = table.get_mut("profiles").and_then(|v| v.as_table_mut()) {
                    profiles.remove(&pname);
                }
                ensure_project_entry(&mut table, &project_name());
                add_profile_to_table(&mut table, &pname, &description, includes, excludes);
                write_toml(&table);
                println!("Updated profile '{}'.", pname);
            }
            "r" => {
                let pname = prompt_input("Profile to remove: ");
                if pname.is_empty() {
                    continue;
                }
                remove_profile(&pname);
            }
            "q" => break,
            _ => println!("Unknown action. Use A, E, R, or Q."),
        }
    }
}

fn interactive_releases_menu() {
    let available = release_names();
    let mut current = current_release_names_in_toml();
    println!("Current releases in dam.toml: {}", if current.is_empty() { "(none)".to_string() } else { current.join(", ") });
    println!("Available release sources: {}", available.join(", "));

    loop {
        let action = prompt_input("Choose [A]dd, [R]emove, [S]elect subset, or [Q]uit: ");
        match action.to_lowercase().as_str() {
            "a" => {
                let name = prompt_input("Release name to add: ");
                if !available.contains(&name) {
                    println!("Release '{}' not found on disk.", name);
                    continue;
                }
                if !current.contains(&name) {
                    current.push(name.clone());
                    apply_release_selection(Some(current.clone()));
                    println!("Added release '{}'.", name);
                } else {
                    println!("Release '{}' is already included.", name);
                }
            }
            "r" => {
                let name = prompt_input("Release name to remove: ");
                if current.contains(&name) {
                    current.retain(|r| r != &name);
                    apply_release_selection(Some(current.clone()));
                    println!("Removed release '{}'.", name);
                } else {
                    println!("Release '{}' is not currently included.", name);
                }
            }
            "s" => {
                let names = prompt_input("Enter comma-separated release names to include: ");
                let selection = names
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                current = selection.clone();
                apply_release_selection(Some(selection));
                println!("Updated release selection.");
            }
            "q" => break,
            _ => println!("Unknown action. Use A, R, S, or Q."),
        }
    }
}

fn interactive_streams_menu() {
    interactive_dam_toml_menu(None, true);
}

fn interactive_sync_menu() {
    regenerate_from_disk(None, true);
}

fn interactive_regenerate() {
    println!("Interactive Regenerate — edit dam.toml, providers, profiles, streams, or sync from reservoir.");

    loop {
        println!("\nSelect an action:");
        println!("  1) Update dam.toml streams/releases");
        println!("  2) Edit providers");
        println!("  3) Edit profiles");
        println!("  4) Edit stream selection");
        println!("  5) Edit releases");
        println!("  6) Sync all from reservoir");
        println!("  q) Quit");
        let choice = prompt_input("Select an option [1]: ");

        match choice.as_str() {
            "" | "1" => interactive_dam_toml_menu(None, false),
            "2" => interactive_provider_menu(),
            "3" => interactive_profile_menu(),
            "4" => interactive_streams_menu(),
            "5" => interactive_releases_menu(),
            "6" => interactive_sync_menu(),
            "q" | "Q" => {
                println!("Exiting regenerate interactive mode.");
                break;
            }
            _ => println!("Unknown selection. Please choose 1,2,3,4,5,6 or q."),
        }
    }
}

fn add_provider_entry(name: &str, sdk: Option<&str>, cli: Option<&str>, any: bool) {
    let mut table = load_toml();
    ensure_project_entry(&mut table, &project_name());
    ensure_provider_block(&mut table);
    let providers = table["provider"].as_table_mut().unwrap();
    let entry = providers
        .entry(name.to_string())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let provider_table = entry.as_table_mut().unwrap();
    provider_table.insert("any".to_string(), toml::Value::Boolean(any));
    if let Some(sdk) = sdk {
        provider_table.insert("sdk".to_string(), toml::Value::String(sdk.to_string()));
    }
    if let Some(cli_version) = cli {
        provider_table.insert(
            "cli".to_string(),
            toml::Value::String(cli_version.to_string()),
        );
    }
    write_toml(&table);
    println!("Updated provider '{}' in dam.toml.", name);
}

pub fn run(command: Option<RegenerateCommands>) {
    match command {
        Some(RegenerateCommands::DamToml {
            command,
            stream,
            force,
        }) => match command {
            Some(DamTomlCommands::Providers {
                name,
                sdk,
                cli,
                any,
            }) => {
                if let Some(name) = name {
                    add_provider_entry(&name, sdk.as_deref(), cli.as_deref(), any);
                } else {
                    interactive_provider_menu();
                }
            }
            Some(DamTomlCommands::Profiles) => interactive_profile_menu(),
            Some(DamTomlCommands::Streams { stream }) => {
                if stream.is_some() {
                    regenerate_from_disk(stream.as_deref(), true);
                } else {
                    interactive_streams_menu();
                }
            }
            Some(DamTomlCommands::Releases { add, remove }) => {
                if add.is_some() || remove.is_some() {
                    let mut current = current_release_names_in_toml();
                    if let Some(add_name) = add {
                        if !current.contains(&add_name) {
                            current.push(add_name);
                        }
                    }
                    if let Some(remove_name) = remove {
                        current.retain(|r| r != &remove_name);
                    }
                    apply_release_selection(Some(current));
                } else {
                    interactive_releases_menu();
                }
            }
            Some(DamTomlCommands::Sync { stream, all }) => {
                if all || stream.is_none() {
                    regenerate_from_disk(None, true);
                } else {
                    regenerate_from_disk(stream.as_deref(), true);
                }
            }
            None => interactive_dam_toml_menu(stream.as_deref(), force),
        },
        Some(RegenerateCommands::Provider {
            name,
            sdk,
            cli,
            any,
        }) => {
            add_provider_entry(&name, sdk.as_deref(), cli.as_deref(), any);
        }
        Some(RegenerateCommands::Sync { stream, all }) => {
            if all || stream.is_none() {
                regenerate_from_disk(None, true);
            } else {
                regenerate_from_disk(stream.as_deref(), true);
            }
        }
        None => {
            interactive_regenerate();
        }
    }
}
