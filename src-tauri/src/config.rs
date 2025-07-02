use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use dirs::config_dir;

#[derive(Serialize,Deserialize, Clone)]
pub struct RecentGameEntry{
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub recent_games: Vec<RecentGameEntry>, // List of game titles
    pub defender_excluded: bool,
    pub game_images: std::collections::HashMap<String, String>,
}

// Get the configuration file path
fn get_config_path() -> PathBuf {
    // Retrieve the base config directory (e.g., AppData\Roaming on Windows)
    let base_dir = config_dir().expect("Failed to locate config directory");

    // Append your app-specific directory and file
    base_dir.join("PirateLand").join("config.json")
}

// Load the configuration
pub fn load_config() -> AppConfig {
    let config_path = get_config_path();
    if config_path.exists() {
        let mut file = File::open(&config_path).expect("Failed to open config file");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("Failed to read config file");
        
        // Handle legacy config format
        let mut config: AppConfig = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => {
                // Try to parse as old format (Vec<String>)
                let legacy: Vec<String> = serde_json::from_str(&content)
                    .unwrap_or_default();
                
                AppConfig {
                    recent_games: legacy.into_iter().map(|name| RecentGameEntry {
                        name,
                        path: String::new() // Set default path or handle differently
                    }).collect(),
                    ..AppConfig::default()
                }
            }
        };
        
        // Migrate any empty paths if needed
        for game in &mut config.recent_games {
            if game.path.is_empty() {
                game.path = "<unknown-path>".to_string();
            }
        }
        
        config
    } else {
        AppConfig::default()
    }
}

pub fn save_game_image_to_config(game_name: &str, image_url: &str) {
    let mut config = load_config();
    config
        .game_images
        .insert(game_name.to_string(), image_url.to_string());
    save_config(&config);
}

// Save the configuration
pub fn save_config(config: &AppConfig) {
    let config_path = get_config_path();

    // Ensure the directory exists
    if let Some(parent_dir) = config_path.parent() {
        fs::create_dir_all(parent_dir).expect("Failed to create config directory");
    }

    let mut file = File::create(&config_path).expect("Failed to create config file");
    let content = serde_json::to_string_pretty(&config).expect("Failed to serialize config");
    file.write_all(content.as_bytes()).expect("Failed to write config file");
}
