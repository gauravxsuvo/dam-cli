pub mod flutter;
pub mod firebase;
pub mod python;
pub mod custom;
pub mod rust;
pub mod js;


pub trait Provider {
    fn name(&self) -> &'static str;
    fn check_environment(&self) -> bool;
    fn default_toml(&self, project_name: &str) -> String;
}

pub fn get_provider(name: &str) -> Box<dyn Provider> {
    match name.to_lowercase().as_str() {
        "flutter" => Box::new(flutter::FlutterProvider),
        "firebase" => Box::new(firebase::FirebaseProvider),
        "python" => Box::new(python::PythonProvider),
        "rust" => Box::new(rust::RustProvider),
        "js" | "javascript" | "node" | "nodejs" => Box::new(js::JsProvider),
        _ => Box::new(custom::CustomProvider),
    }
}

// Return a set of allowed/trusted runtime commands for a provider. These are commands
// that can be executed automatically without treating them as untrusted.
pub fn allowed_commands_for(name: &str) -> Vec<String> {
    match name.to_lowercase().as_str() {
        "flutter" => vec!["flutter pub get".to_string(), "flutter clean".to_string(), "flutter build".to_string()],
        "python" => vec!["pip install -r requirements.txt".to_string(), "poetry install".to_string()],
        "rust" => vec!["cargo build".to_string(), "cargo test".to_string(), "cargo fmt".to_string(),"cargo check".to_string()],
        "js" | "javascript" | "node" | "nodejs" => vec!["npm install".to_string(), "npm run build".to_string(), "yarn install".to_string(), "yarn build".to_string()],
        _ => vec![],
    }
}