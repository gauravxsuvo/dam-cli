use super::Provider;
use std::process::Command;

pub struct JsProvider;

impl Provider for JsProvider {
    fn name(&self) -> &'static str {
        "js"
    }

    fn check_environment(&self) -> bool {
        println!("Checking Node.js environment...");
        Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
    }

    fn default_toml(&self, project_name: &str) -> String {
        format!(r#"[project]
name = "{}"
provider = "js"
enforce_password_on_project_import = false
description = "A JavaScript/Node project managed by Dam"

[setup]
commands = [
    "npm install",
    "npm run build"
]

# Note: permitted commands are enforced in code; do not place them here.

[profiles.full]
description = "Full Admin Access"
include = ["**/*"]
exclude = [".git/**", "node_modules/**", ".dam/**"]

[profiles.contributor]
description = "Contributor Access (No secrets)"
include = ["**/*"]
exclude = [".git/**", "node_modules/**", ".dam/**", "**/*.env"]

streams = ["main"]
releases = []
stable = []
"#, project_name)
    }
}
