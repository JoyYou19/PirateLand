use config::{load_config, save_config, save_game_image_to_config, RecentGameEntry};
use scraper::{Html, Selector};
use serde::Deserialize;
use steamapi::{fetch_game_details, load_steam_games, GameDetails, SteamApp, SteamGameStore};
use tauri::{Window, WindowEvent};
use tokio::runtime::Runtime;
use scrapers::scrape_games;
use tokio::sync::Mutex;
use std::process::Child;
use std::{collections::HashMap, io::{self, BufRead}, path::PathBuf, process::{Command, Stdio}, sync::Arc};
use std::fs;
use std::path::Path;
use reqwest::Client;
use lazy_static::lazy_static;

lazy_static! {
    static ref GO_SERVER_PROCESS: Arc<std::sync::Mutex<Option<Child>>> = Arc::new(std::sync::Mutex::new(None));
}

// Shared authenticated client
static AUTH_CLIENT: once_cell::sync::Lazy<Arc<Mutex<Option<auth_and_download::AuthenticatedClient>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

static STEAM_GAME_STORE: once_cell::sync::Lazy<Arc<Mutex<SteamGameStore>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(SteamGameStore::new())));
// The main lib file that is the main entry for the app
mod scrapers;
mod proxy;
mod auth_and_download;
mod steamapi;
mod config;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn fetch_games(page: usize) -> Result<Vec<scrapers::Game>, String>{
    scrape_games(page).map_err(|e| e.to_string())
}

#[tauri::command]
async fn authenticate(cf_clearance: String, php_sessid: String) -> Result<String, String> {
    let client = auth_and_download::AuthenticatedClient::new(&cf_clearance, &php_sessid)
        .await
        .map_err(|e| e.to_string())?;

    // Store the authenticated client
    let mut auth_client = AUTH_CLIENT.lock().await;
    *auth_client = Some(client);

    Ok("Authentication successful!".to_string())
}

#[tauri::command]
async fn check_defender_exclusion() -> Result<bool, String> {
    let config = load_config();
    Ok(config.defender_excluded)
}

// Add a helper function to update the Defender exclusion status
#[tauri::command]
async fn set_defender_exclusion_status(status: bool) -> Result<(), String> {
    let mut config = load_config();
    config.defender_excluded = status;
    save_config(&config);
    Ok(())
}

#[tauri::command]
async fn open_folder(folder_path: String) -> Result<(), String> {
    let path = PathBuf::from(folder_path.clone());

    if !path.exists() {
        return Err(format!("Folder does not exist: {}", folder_path));
    }

#[cfg(target_os = "windows")]
    Command::new("cmd")
        .args(&["/c", "start", "", &path.to_string_lossy()])
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;

    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;

    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn drop_torrent(torrent_file_path: String) -> Result<String,String>{
    let mut auth_client = AUTH_CLIENT.lock().await;
    if let Some(client) = auth_client.as_mut() {
        let _ = client.drop_torrent(&torrent_file_path, "").await;
        return Ok(format!("Torrent dropped successfully"));
    }
    return Err(format!(
        "Failed to drop torrent",
    ));
}

#[tauri::command]
async fn download_torrent(game_title: String) -> Result<String, String> {
    let mut auth_client = AUTH_CLIENT.lock().await;
    if let Some(client) = auth_client.as_mut() {
        println!("Preparing to download torrent for game: {}", game_title);

        // Ensure the `online_fix_auth` cookie is fetched for the game
        match client.fetch_online_fix_auth(&game_title).await {
            Ok(_) => {
                println!("Successfully fetched `online_fix_auth` cookie for {}", game_title);
            }
            Err(e) => {
                println!(
                    "Failed to fetch `online_fix_auth` cookie for {}: {}",
                    game_title, e
                );
                return Err(format!(
                    "Failed to fetch `online_fix_auth` cookie: {}",
                    e
                ));
            }
        }

        // Download the torrent file
        match client.download_torrent(&game_title).await {
            Ok(_) => {
                println!("Torrent downloaded successfully");
                Ok(format!("Torrent downloaded successfully"))
            }
            Err(e) => {
                println!(
                    "Failed to download torrent for {}: {}",
                    game_title, e
                );
                Err(format!("Failed to download torrent: {}", e))
            }
        }
    } else {
        println!("No authenticated client found. Please authenticate first.");
        Err("No authenticated client found. Please authenticate first.".to_string())
    }
}

#[tauri::command]
async fn find_and_get_game_details(query: String) -> Result<Option<GameDetails>, String> {
    // Lock the global Steam game store and perform a fuzzy search
    let game_store = STEAM_GAME_STORE.lock().await;
    if let Some(game) = game_store.fuzzy_search(&query) {
        // Fetch the game details using the AppID
        match fetch_game_details(game.appid).await {
            Ok(details) => Ok(Some(details)),
            Err(err) => Err(format!(
                "Failed to fetch game details for '{}': {}",
                game.name, err
            )),
        }
    } else {
        Err(format!("No game found matching '{}'", query))
    }
}

#[tauri::command]
async fn find_and_get_game_details_library(query: String) -> Result<Option<GameDetails>, String> {
    // Load the config to check for cached game images
    let config = load_config();

    if let Some(image_url) = config.game_images.get(&query) {
        // Return cached details with defaults for other fields
        let mut game_details = GameDetails::default();
        game_details.header_image = Some(image_url.to_string());
        return Ok(Some(game_details));
    }

    // Lock the global Steam game store and perform a fuzzy search
    let game_store = STEAM_GAME_STORE.lock().await;
    if let Some(game) = game_store.fuzzy_search(&query) {
        // Fetch the game details using the AppID
        match fetch_game_details(game.appid).await {
            Ok(details) => {
                // Save the image URL to the config
                if let Some(ref header_image) = details.header_image {
                    save_game_image_to_config(&query, header_image);
                }
                Ok(Some(details))
            }
            Err(err) => Err(format!(
                "Failed to fetch game details for '{}': {}",
                game.name, err
            )),
        }
    } else {
        Err(format!("No game found matching '{}'", query))
    }
}

#[tauri::command]
async fn get_downloads() -> Result<Vec<DownloadProgress>, String> {
    let response = reqwest::get("http://localhost:8091/downloads-progress")
        .await
        .map_err(|e| e.to_string())?;

    let downloads: Vec<DownloadProgress> = response
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(downloads)
}

#[derive(serde::Serialize, Deserialize, Debug)]
struct DownloadProgress {
    name: String,
    progress: f64,         // Value between 0.0 and 1.0
    speed_mb_ps: f64,      // Download speed in MB/s
    peers_connected: u32,  // Number of connected peers
    downloaded: i64,
    total_size: i64,
    extract_progress: f64,
    eta: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Create a Tokio runtime
    let runtime = Runtime::new().expect("Failed to create Tokio runtime");
    // Load Steam games synchronously before proceeding
    runtime.block_on(async {
        match load_steam_games("steam_games.json").await {
            Ok(game_store) => {
                let mut global_store = STEAM_GAME_STORE.lock().await;
                *global_store = game_store.lock().await.to_owned();
                println!("Successfully loaded Steam games into global storage.");
            }
            Err(err) => {
                eprintln!("Failed to load Steam games: {}", err);
            }
        }
    });
    create_default_directories();
    // Start the Go server and store its handle
    let go_server_handle = start_go_server().expect("Failed to start Go server");
    {
        let mut go_server = GO_SERVER_PROCESS.lock().unwrap(); // No .await required
        *go_server = Some(go_server_handle);
    }
    // Spawn the proxy server inside the runtime
    runtime.spawn(async {
        proxy::start_proxy().await;
    });


    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet,
            fetch_games,
            authenticate,
            download_torrent,
            find_and_get_game_details,
            find_and_get_game_details_library,
            get_downloads,
            get_installed_games,
            open_folder,
            update_recent_games,
            get_recent_games,
            search_online_fix,
            exclude_folder_in_defender,
            check_defender_exclusion,
            set_defender_exclusion_status,
            drop_torrent,
            uninstall_game,
        ])
        .on_window_event(|window: &Window, event: &WindowEvent| {
            if let WindowEvent::CloseRequested { .. } = event {
                // When the window is closed, stop the Go server
                let mut go_server = GO_SERVER_PROCESS.lock().unwrap();
                if let Some(mut process) = go_server.take() {
                    let _ = process.kill(); // Kill the process
                    println!("Go server process killed");
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn update_recent_games(name: String, path: String) {
    let mut config = load_config();
    let new_game = RecentGameEntry { name, path };
    
    // Remove existing entries with the same name or path
    config.recent_games.retain(|g| g.name != new_game.name && g.path != new_game.path);
    
    config.recent_games.insert(0, new_game);
    config.recent_games.truncate(3);
    save_config(&config);
}

#[tauri::command]
fn get_recent_games() -> Vec<RecentGameEntry> {
    load_config().recent_games
}

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn start_go_server() -> io::Result<Child> {
    let go_server_binary = if cfg!(target_os = "windows") {
        "./go-rain-server/go-rain-server.exe"
    } else {
        "./go-rain-server/go-rain-server"
    };

    let mut command = Command::new(go_server_binary);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command.spawn()?; // Spawn the process and return the child handle
    Ok(child)
}

fn create_default_directories() {
    // Default directories
    let torrents_dir = if cfg!(target_os = "windows") {
        format!("{}/PirateLand/torrents", std::env::var("APPDATA").unwrap())
    } else {
        format!("{}/.pirateland/torrents", std::env::var("HOME").unwrap())
    };

    let downloads_dir = if cfg!(target_os = "windows") {
        format!("{}/Downloads/PirateLand", std::env::var("USERPROFILE").unwrap())
    } else {
        format!("{}/Downloads/PirateLand", std::env::var("HOME").unwrap())
    };

    // Create directories if they don't exist
    for dir in &[&torrents_dir, &downloads_dir] {
        if !Path::new(dir).exists() {
            fs::create_dir_all(dir).expect(&format!("Failed to create directory: {}", dir));
        }
    }

    println!("Torrents directory: {}", torrents_dir);
    println!("Downloads directory: {}", downloads_dir);
}

#[derive(serde::Serialize)]
struct InstalledGame {
    name: String,       // The name of the game folder
    path: String,       // The absolute path to the game
}

#[tauri::command]
async fn get_installed_games() -> Result<Vec<InstalledGame>, String> {
    use std::fs;
    use std::path::PathBuf;

    // Define the games directory
    let games_dir = format!("{}/Downloads/PirateLand", std::env::var("USERPROFILE").unwrap());

    // Check if the directory exists
    let dir_path = PathBuf::from(&games_dir);
    if !dir_path.exists() {
        return Ok(vec![]); // Return an empty list if the directory doesn't exist
    }

// Read all subdirectories in the games directory
    let mut installed_games = Vec::new();
    match fs::read_dir(dir_path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                            // Construct the "Extracted" subfolder path
                            let extracted_path = path.join("Extracted");

                            // Check if the "Extracted" subfolder exists
                            if extracted_path.exists() && extracted_path.is_dir() {
                                installed_games.push(InstalledGame {
                                    name: folder_name.to_string(),
                                    path: extracted_path.to_string_lossy().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Err(e) => return Err(format!("Failed to read games directory: {}", e)),
    }

    Ok(installed_games)
}

#[tauri::command]
async fn uninstall_game(game_path: String) -> Result<(), String> {
    // Define the parent directory
    let games_dir = format!("{}/Downloads/PirateLand", std::env::var("USERPROFILE").unwrap());
    let games_dir_path = PathBuf::from(&games_dir);

    // Ensure the game path is within the games directory for safety
    let target_path = PathBuf::from(&game_path);
    if !target_path.starts_with(&games_dir_path) {
        return Err("The specified path is not within the allowed directory.".to_string());
    }

    // Get the parent directory of the game folder (the folder containing the game)
    let parent_dir = target_path.parent().ok_or_else(|| {
        "Unable to determine the parent directory of the specified path.".to_string()
    })?;

    // Check if the parent directory exists and is within the allowed games directory
    if !parent_dir.starts_with(&games_dir_path) {
        return Err("The parent directory is not within the allowed directory.".to_string());
    }

    if parent_dir.exists() && parent_dir.is_dir() {
        // Attempt to remove the parent directory recursively
        match fs::remove_dir_all(parent_dir) {
            Ok(_) => Ok(()), // Successfully deleted
            Err(e) => Err(format!("Failed to delete the directory: {}", e)), // Error while deleting
        }
    } else {
        Err("The specified directory does not exist.".to_string()) // Path not found
    }
}

#[tauri::command]
async fn search_online_fix(query: String) -> Result<Vec<String>, String> {
    let client = Client::new();
    let url = "https://online-fix.me/engine/ajax/search.php";

    // Form data
    let mut form_data = HashMap::new();
    form_data.insert("query", query);

    // Perform the POST request
    match client
        .post(url)
        .form(&form_data)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0")
        .header("Accept", "*/*")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .header("X-Requested-With", "XMLHttpRequest")
        .header("Origin", "https://online-fix.me")
        .header("Referer", "https://online-fix.me/")
        .header("Cookie", "dle_user_id=1956012; dle_password=5635960d1de3680613ba0cab35bfcf56; PHPSESSID=11maut6bo4jo1iqn33m9j16a2t; cf_clearance=hYmrGSrcAoxZffr4gVmG7yDDJolNYHDlpQHt4v8stVo-1732159904-1.2.1.1-nzA.D7ojQtVRa2fcTA740sQ; e7f652f7be_delayCount=4; e7f652f7be_blockTimer=1; u_e7f652f7be=1")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.text().await {
                    Ok(text) => {
                        // Parse the response HTML
                        let document = Html::parse_document(&text);

                        // Create a selector for <span class="searchheading">
                        let selector = Selector::parse("span.searchheading").unwrap();

                        // Extract the text content and apply the modification
                        let results: Vec<String> = document
                            .select(&selector)
                            .filter_map(|element| element.text().next().map(|text| {
                                let mut title = text.trim().to_string();

                                // Remove the last 7 characters (safely for UTF-8)
                                if title.chars().count() > 8 {
                                    title = title
                                        .chars()
                                        .take(title.chars().count() - 8)
                                        .collect::<String>();
                                }

                                title
                            }))
                            .collect();

                        Ok(results) // Return the modified game names
                    }
                    Err(err) => Err(format!("Failed to read response body: {}", err)),
                }
            } else {
                Err(format!("Request failed with status: {}", response.status()))
            }
        }
        Err(err) => Err(format!("Failed to make request: {}", err)),
    }
}

#[tauri::command]
fn exclude_folder_in_defender() -> Result<String, String> {
    let folder_path = format!("{}/Downloads/PirateLand", std::env::var("USERPROFILE").unwrap());
    let script = format!(
        "Add-MpPreference -ExclusionPath \"{}\"",
        folder_path
    );

    let output = Command::new("powershell")
        .arg("-Command")
        .arg(script)
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                Ok(format!(
                    "Successfully excluded the folder '{}' from Windows Defender scans.",
                    folder_path
                ))
            } else {
                Err(format!(
                    "Failed to exclude the folder. Error: {}",
                    String::from_utf8_lossy(&result.stderr)
                ))
            }
        }
        Err(err) => Err(format!("Failed to execute PowerShell: {}", err)),
}
}
