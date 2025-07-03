use config::{load_config, save_config, save_game_image_to_config, RecentGameEntry};
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use scrapers::scrape_games;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Child;
use std::time::Duration;
use std::{
    collections::HashMap,
    io::{self, BufRead},
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};
use steamapi::{
    fetch_game_details, load_steam_games, GameDetails, SteamApp, SteamGame, SteamGameStore,
    SteamGameStoreIndex,
};
use tauri::{Window, WindowEvent};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::time::sleep;

// Shared authenticated client
static AUTH_CLIENT: once_cell::sync::Lazy<
    Arc<Mutex<Option<auth_and_download::AuthenticatedClient>>>,
> = once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

static STEAM_GAME_STORE: once_cell::sync::Lazy<Arc<Mutex<SteamGameStore>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(SteamGameStore::new())));
use torrent_manager::TorrentManager;

static TORRENT_MANAGER: Lazy<Arc<Mutex<Option<Arc<TorrentManager>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

pub static STEAM_GAME_STORE_INDEX: Lazy<Arc<Mutex<SteamGameStoreIndex>>> =
    Lazy::new(|| Arc::new(Mutex::new(SteamGameStoreIndex::new())));
// The main lib file that is the main entry for the app
mod auth_and_download;
mod config;
mod proxy;
mod scrapers;
mod steamapi;
mod torrent_manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn fetch_games(page: usize) -> Result<Vec<scrapers::Game>, String> {
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
    #[cfg(not(target_os = "windows"))]
    {
        return Ok(true); // on Linux/macOS, just return true
    }

    #[cfg(target_os = "windows")]
    {
        let config = load_config();
        return Ok(config.defender_excluded);
    }
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
async fn drop_torrent(id: u64) -> Result<(), String> {
    let manager = TORRENT_MANAGER.lock().await;
    if let Some(manager) = manager.as_ref() {
        manager
            .remove_torrent(id)
            .await
            .map_err(|e| format!("Failed to remove torrent: {}", e))
    } else {
        Err("Torrent manager not initialized".to_string())
    }
}

#[tauri::command]
async fn download_torrent(game_title: String) -> Result<String, String> {
    let mut auth_client = AUTH_CLIENT.lock().await;
    if let Some(client) = auth_client.as_mut() {
        println!("Preparing to download torrent for game: {}", game_title);

        client
            .fetch_online_fix_auth(&game_title)
            .await
            .map_err(|e| {
                format!(
                    "Failed to fetch online_fix_auth cookie for {}: {}",
                    game_title, e
                )
            })?;

        client
            .download_torrent(&game_title)
            .await
            .map_err(|e| format!("Failed to download torrent for {}: {}", game_title, e))?;

        Ok("Torrent added successfully".to_string())
    } else {
        Err("Not authenticated".to_string())
    }
}

#[tauri::command]
async fn download_igggames(url: String, game_title: String) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;

    println!("[DEBUG] Starting download process for URL: {}", url);

    // Step 1: Get the game page HTML
    let game_page = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch game page: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read game page: {}", e))?;

    println!("[DEBUG] Successfully fetched game page HTML");

    // Process HTML in a blocking task
    let download_page_url = tokio::task::spawn_blocking(move || {
        println!("[DEBUG] Parsing game page HTML for download link");
        let document = Html::parse_document(&game_page);
        let selector =
            Selector::parse("p.uk-card a").map_err(|e| format!("Selector error: {}", e))?;

        let elements: Vec<_> = document.select(&selector).collect();
        println!(
            "[DEBUG] Found {} elements matching selector 'p.uk-card a'",
            elements.len()
        );

        let href = elements
            .first()
            .and_then(|element| element.value().attr("href"))
            .ok_or_else(|| {
                println!("[ERROR] No download link found in game page");
                "Download link not found in game page".to_string()
            })?;

        println!("[DEBUG] Found download page URL: {}", href);
        Ok::<String, String>(href.to_string())
    })
    .await
    .map_err(|e| format!("Blocking task failed: {}", e))?
    .map_err(|e| e)?;

    println!("[DEBUG] Found download page URL: {}", download_page_url);

    // Step 2: Fetch the download page HTML
    let download_page = client
        .get(&download_page_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch download page: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read download page: {}", e))?;

    println!("[DEBUG] Successfully fetched download page HTML");

    // Step 3: Extract the long string from JavaScript
    let long_string = extract_long_string(&download_page)?;
    println!(
        "[DEBUG] Extracted long string ({} chars): {}",
        long_string.len(),
        long_string,
    );

    // Step 4: Process the string like JavaScript does
    let processed_string = process_string_javascript_style(&long_string);
    println!(
        "[DEBUG] Processed string ({} chars): {}",
        processed_string.len(),
        &processed_string
    );

    // Step 5: Construct API URL
    let api_url = format!(
        "https://dl1.gamedownloadurl.autos/get-url.php?url={}",
        processed_string
    );
    println!("[DEBUG] API URL: {}", api_url);

    // Wait 5 seconds as the site requires
    //println!("[DEBUG] Waiting 5 seconds before API request");
    //sleep(Duration::from_secs(5)).await;

    // Step 6: Fetch the magnet link
    let magnet_response = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch API: {}", e))?
        .text()
        .await
        .map_err(|e| format!("Failed to read API response: {}", e))?;

    println!("[DEBUG] API response: {}", magnet_response);

    // Step 7: Extract magnet link from response
    let magnet_link = extract_magnet_link(&magnet_response)?;
    println!("[SUCCESS] Found magnet link: {}", magnet_link);

    // ✅ Add torrent directly instead of sending to Go server
    // Get a clone of the torrent manager without holding the lock
    let manager_clone = {
        let manager = TORRENT_MANAGER.lock().await;
        if let Some(m) = manager.as_ref() {
            m.clone()
        } else {
            return Err("Torrent manager not initialized".into());
        }
    };

    // Use the clone to add the torrent
    manager_clone
        .add_torrent_magnet(&magnet_link, &game_title)
        .await
        .map_err(|e| format!("Failed to add torrent: {}", e))?;
    Ok("Download started".to_string())
}

// Helper functions

fn extract_long_string(html: &str) -> Result<String, String> {
    // Find the generateDownloadUrl function
    let re = Regex::new(r"function generateDownloadUrl\(\)\{[^}]*let [^=]+='([^']+)'").unwrap();

    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| "Long string not found in JavaScript".to_string())
}

fn process_string_javascript_style(s: &str) -> String {
    let len = s.len();
    let half = len / 2;
    let mut result = String::new();

    // First part: from (half - 5) to 0, stepping backwards by 2
    let mut i = if half >= 5 { half - 5 } else { 0 };
    while i < len {
        // Ensure we don't go out of bounds
        if let Some(c) = s.chars().nth(i) {
            result.push(c);
        }
        if i < 2 {
            break;
        } // Prevent underflow
        i -= 2;
    }

    // Second part: from (half + 4) to end, stepping by 2
    let mut i = half + 4;
    while i < len {
        if let Some(c) = s.chars().nth(i) {
            result.push(c);
        }
        i += 2;
    }

    result
}

fn extract_magnet_link(html: &str) -> Result<String, String> {
    // Look for the magnetLink variable in the JavaScript
    let re =
        Regex::new(r#"let magnetLink = "([^"]+)""#).map_err(|e| format!("Regex error: {}", e))?;

    re.captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| "Magnet link not found in JavaScript".to_string())
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

#[derive(serde::Serialize, Deserialize, Debug)]
pub struct DownloadProgress {
    id: u64,
    name: String,
    progress: f64,
    speed_mb_ps: f64,
    peers_connected: u32,
    downloaded: i64,
    total_size: i64,
    extract_progress: f64,
    eta: String,
}

#[tauri::command]
async fn get_downloads() -> Result<Vec<DownloadProgress>, String> {
    let manager = TORRENT_MANAGER.lock().await;
    if let Some(manager) = manager.as_ref() {
        let active = manager.list_torrents().await;
        if active.is_empty() {
            // No active downloads or extractions
            Ok(Vec::new())
        } else {
            Ok(active)
        }
    } else {
        Ok(Vec::new())
    }
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

        let mut global_store = STEAM_GAME_STORE_INDEX.lock().await;
        global_store
            .load_games("games_index.json")
            .await
            .expect("Failed to load steam games");

        // Initialize torrent manager with platform-specific download directory
        let downloads_dir = if cfg!(target_os = "windows") {
            format!(
                "{}/Downloads/PirateLand",
                std::env::var("USERPROFILE").unwrap()
            )
        } else {
            // Linux: Use ~/Downloads/PirateLand or XDG_DOWNLOAD_DIR if set
            let base_dir = std::env::var("XDG_DOWNLOAD_DIR")
                .unwrap_or_else(|_| format!("{}/Downloads", std::env::var("HOME").unwrap()));
            format!("{}/PirateLand", base_dir)
        };

        // Create the directory if it doesn't exist
        tokio::fs::create_dir_all(&downloads_dir)
            .await
            .expect("Failed to create download directory");

        let manager = TorrentManager::new(downloads_dir.into())
            .await
            .expect("Failed to create torrent manager");
        *TORRENT_MANAGER.lock().await = Some(Arc::new(manager));
    });
    create_default_directories();
    // Spawn the proxy server inside the runtime
    runtime.spawn(async {
        proxy::start_proxy().await;
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            greet,
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
            search_igggames,
            download_igggames,
            fetch_games_index,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn fetch_games_index(category: String, page: usize, page_size: usize) -> Vec<SteamGame> {
    let store = STEAM_GAME_STORE_INDEX.lock().await;
    store.get_games(&category, page, page_size).await
}

#[tauri::command]
fn update_recent_games(name: String, path: String) {
    let mut config = load_config();
    let new_game = RecentGameEntry { name, path };

    // Remove existing entries with the same name or path
    config
        .recent_games
        .retain(|g| g.name != new_game.name && g.path != new_game.path);

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
        format!(
            "{}/Downloads/PirateLand",
            std::env::var("USERPROFILE").unwrap()
        )
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
    name: String, // The name of the game folder
    path: String, // The absolute path to the game
}

#[tauri::command]
async fn get_installed_games() -> Result<Vec<InstalledGame>, String> {
    use std::fs;
    use std::path::PathBuf;

    let games_dir = if cfg!(target_os = "windows") {
        format!(
            "{}/Downloads/PirateLand",
            std::env::var("USERPROFILE").unwrap_or_else(|_| {
                println!("[DEBUG] USERPROFILE env var missing");
                ".".to_string()
            })
        )
    } else {
        let base_dir = std::env::var("XDG_DOWNLOAD_DIR").unwrap_or_else(|_| {
            format!(
                "{}/Downloads",
                std::env::var("HOME").unwrap_or_else(|_| {
                    println!("[DEBUG] HOME env var missing");
                    ".".to_string()
                })
            )
        });
        format!("{}/PirateLand", base_dir)
    };

    println!("[DEBUG] Games directory: {}", games_dir);
    let dir_path = PathBuf::from(&games_dir);
    if !dir_path.exists() {
        println!("[DEBUG] Games directory does not exist.");
        return Ok(vec![]);
    }

    let mut installed_games = Vec::new();

    match fs::read_dir(&dir_path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    println!("[DEBUG] Found folder: {}", path.display());

                    if path.is_dir() {
                        if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                            let extracted_path = path.join("Extracted");
                            let game_path = if extracted_path.exists() && extracted_path.is_dir() {
                                println!("[DEBUG] Found Extracted folder inside {}", folder_name);
                                extracted_path
                            } else {
                                println!(
                                    "[DEBUG] No Extracted folder, using folder itself: {}",
                                    folder_name
                                );
                                path.clone()
                            };

                            println!("[DEBUG] Checking for game files in {}", game_path.display());

                            let has_game_files = fs::read_dir(&game_path)
                                .ok()
                                .map(|entries| {
                                    entries.filter_map(|e| e.ok()).any(|e| {
                                        let p = e.path();
                                        if p.is_file() {
                                            let ext = p
                                                .extension()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or("")
                                                .to_lowercase();
                                            let is_game_file = matches!(
                                                ext.as_str(),
                                                "exe" | "so" | "bin" | "appimage" | "sh"
                                            );

                                            println!(
                                                "[DEBUG] Found file: {} (ext: {}) => game file? {}",
                                                p.display(),
                                                ext,
                                                is_game_file
                                            );

                                            is_game_file
                                        } else {
                                            false
                                        }
                                    })
                                })
                                .unwrap_or(false);

                            println!("[DEBUG] Has game files: {}", has_game_files);

                            if has_game_files {
                                installed_games.push(InstalledGame {
                                    name: folder_name.to_string(),
                                    path: game_path.to_string_lossy().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to read games directory: {}", e);
            println!("[DEBUG] {}", err_msg);
            return Err(err_msg);
        }
    }

    println!("[DEBUG] Installed games found: {}", installed_games.len());
    Ok(installed_games)
}

#[tauri::command]
async fn uninstall_game(game_path: String) -> Result<(), String> {
    // Define the parent directory
    let games_dir = format!(
        "{}/Downloads/PirateLand",
        std::env::var("USERPROFILE").unwrap()
    );
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

#[derive(Debug, Serialize)]
struct SearchResult {
    title: String,
    url: String,
    source: String,
}

#[tauri::command]
async fn search_igggames(query: String) -> Result<Vec<SearchResult>, String> {
    let client = Client::new();
    let url = format!("https://pcgamestorrents.com/?s={}", query.replace(' ', "+"));

    match client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.text().await {
                    Ok(html) => {
                        let document = Html::parse_document(&html);

                        // Selector for each search result article
                        let article_selector = Selector::parse("article").unwrap();
                        // Selector for title within the article
                        let title_selector = Selector::parse(".uk-article-title").unwrap();
                        // Selector for URL within the article
                        let link_selector = Selector::parse("a.uk-link-reset").unwrap();

                        let mut results = Vec::new();

                        for element in document.select(&article_selector) {
                            if let Some(title_element) = element.select(&title_selector).next() {
                                if let Some(link_element) = element.select(&link_selector).next() {
                                    if let Some(href) = link_element.value().attr("href") {
                                        // Extract and clean the title text
                                        let title = title_element.text().collect::<String>()
                                            .trim()
                                            .replace('\u{201c}', "")  // Remove left double quote
                                            .replace('\u{201d}', "")  // Remove right double quote
                                            .to_string();

                                        results.push(SearchResult {
                                            title,
                                            url: href.to_string(),
                                            source: "igggames".to_string(),
                                        });
                                    }
                                }
                            }
                        }

                        if results.is_empty() {
                            Err("No search results found".into())
                        } else {
                            Ok(results)
                        }
                    }
                    Err(err) => Err(format!("Failed to read response: {}", err)),
                }
            } else {
                Err(format!("HTTP error: {}", response.status()))
            }
        }
        Err(err) => Err(format!("Request failed: {}", err)),
    }
}

#[tauri::command]
async fn search_online_fix(query: String) -> Result<Vec<SearchResult>, String> {
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
                        let results: Vec<SearchResult> = document
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

                                SearchResult { title: title, url: "none".to_string(), source: "online-fix".to_string() }
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
    let folder_path = format!(
        "{}/Downloads/PirateLand",
        std::env::var("USERPROFILE").unwrap()
    );
    let script = format!("Add-MpPreference -ExclusionPath \"{}\"", folder_path);

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
