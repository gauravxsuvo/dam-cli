use super::Provider;
use std::process::Command;

pub struct RustProvider;

impl Provider for RustProvider {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn check_environment(&self) -> bool {
        println!("Checking Rust environment...");
        Command::new("rustc").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    fn default_toml(&self, project_name: &str) -> String {
        format!(r#"[project]
name = "{}"
provider = "rust"
enforce_password_on_project_import = false
description = "A Rust project managed by Dam"

[setup]
commands = [
    "cargo build",
    "cargo test"
]

# Note: permitted commands are enforced in code; do not place them here.

[profiles.full]
description = "Full Admin Access"
include = ["**/*"]
exclude = [".git/**", "target/**", ".dam/**"]

[profiles.contributor]
description = "Contributor Access (No secrets)"
include = ["**/*"]
exclude = [".git/**", "target/**", ".dam/**", "**/*.env"]

streams = ["main"]
releases = []
stable = []
"#, project_name)
    }
}
