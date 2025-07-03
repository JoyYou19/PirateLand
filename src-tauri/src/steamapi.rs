use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use strsim::jaro_winkler;
use tokio::sync::Mutex;

// Define the structure to parse the API response
#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct SteamApp {
    pub appid: u32,
    pub name: String,
    #[serde(skip)]
    pub name_lower: String,
}

/// Trie Node structure
#[derive(Default, Debug, Clone)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    game_index: Option<usize>, // Index of the game in the main game list
}

/// Trie structure
#[derive(Default, Debug, Clone)]
struct Trie {
    root: TrieNode,
}

// Steam API Response Structure
#[derive(Serialize, Deserialize, Debug)]
pub struct GameDetailsResponse {
    #[serde(rename = "data")]
    pub data: Option<GameDetails>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct GameDetails {
    pub name: Option<String>,
    pub short_description: Option<String>,
    pub header_image: Option<String>,
    pub developers: Option<Vec<String>>,
    pub publishers: Option<Vec<String>>,
    pub price_overview: Option<PriceOverview>,
    pub detailed_description: Option<String>,
    pub about_the_game: Option<String>,
    pub screenshots: Option<Vec<Screenshot>>,
    pub genres: Option<Vec<Genre>>,
    pub pc_requirements: Option<PCRequirements>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PriceOverview {
    pub currency: Option<String>,
    pub discount_percent: Option<u32>,
    pub final_: Option<u32>, // Use `final_` since `final` is a reserved keyword
    pub final_formatted: Option<String>,
    pub initial: Option<u32>,
    pub initial_formatted: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Screenshot {
    pub id: Option<u32>,
    pub path_full: Option<String>,
    pub path_thumbnail: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Genre {
    pub id: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct PCRequirements {
    pub minimum: Option<String>,
    pub recommended: Option<String>,
}

pub async fn fetch_game_details(appid: u32) -> Result<GameDetails, Box<dyn std::error::Error>> {
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        appid
    );
    let response = reqwest::get(&url).await?;
    let response_json: serde_json::Value = response.json().await?;

    // Print the raw JSON response for debugging

    // Parse the response for the specific appid
    if response_json[appid.to_string()]["success"]
        .as_bool()
        .unwrap_or(false)
    {
        let data = &response_json[appid.to_string()]["data"];

        // Print the extracted data for additional context

        // Deserialize into the GameDetails struct
        let details: GameDetails = serde_json::from_value(data.clone())?;
        Ok(details)
    } else {
        Err(format!("Failed to fetch game details for appid: {}", appid).into())
    }
}

impl Trie {
    fn new() -> Self {
        Self {
            root: TrieNode::default(),
        }
    }

    fn insert(&mut self, word: &str, game_index: usize) {
        let mut current = &mut self.root;
        for ch in word.chars() {
            current = current.children.entry(ch).or_insert_with(TrieNode::default);
        }
        current.game_index = Some(game_index);
    }

    fn search(&self, word: &str) -> Option<usize> {
        let mut current = &self.root;
        for ch in word.chars() {
            match current.children.get(&ch) {
                Some(child) => current = child,
                None => return None,
            }
        }
        current.game_index
    }
}

/// Global storage for Steam games and Trie
#[derive(Default, Clone)]
pub struct SteamGameStore {
    games: Vec<SteamApp>,
    trie: Trie,
}

impl SteamGameStore {
    pub fn new() -> Self {
        Self {
            games: Vec::new(),
            trie: Trie::new(),
        }
    }

    pub fn load_games(&mut self, games: Vec<SteamApp>) {
        self.games = games;
        for (i, game) in self.games.iter().enumerate() {
            self.trie.insert(&game.name_lower, i);
        }
    }

    pub fn fuzzy_search(&self, query: &str) -> Option<&SteamApp> {
        let query_lower = query.to_lowercase();

        // Attempt exact match using the Trie
        if let Some(index) = self.trie.search(&query_lower) {
            return self.games.get(index);
        }

        // Fallback to fuzzy search if no exact match is found
        let mut best_match: Option<&SteamApp> = None;
        let mut highest_score = 0.0;

        for game in &self.games {
            let score = jaro_winkler(&game.name_lower, &query_lower);
            if score > highest_score {
                highest_score = score;
                best_match = Some(game);
            }
        }

        best_match
    }
}

#[derive(Deserialize)]
struct SteamGamesFile {
    #[serde(flatten)]
    games: HashMap<String, SteamApp>,
}

/// Asynchronously load Steam games into the global store
pub async fn load_steam_games(
    file_path: &str,
) -> Result<Arc<Mutex<SteamGameStore>>, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    // Deserialize into the wrapper struct
    let wrapper: SteamGamesFile = serde_json::from_reader(reader)?;

    // Extract the games and convert to Vec
    let mut games: Vec<SteamApp> = wrapper.games.into_values().collect();

    // Compute `name_lower` for each game
    for game in &mut games {
        game.name_lower = game.name.to_lowercase();
    }

    // Initialize the global game store
    let game_store = Arc::new(Mutex::new(SteamGameStore::new()));
    {
        let mut store = game_store.lock().await;
        store.load_games(games);
    }

    Ok(game_store)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamGame {
    pub appid: String,
    pub name: String,
    pub header_image: String,
    pub recommendations: u32,
    pub positive: u32,
    pub negative: u32,
}

pub struct SteamGameStoreIndex {
    pub games: Arc<Mutex<HashMap<String, SteamGame>>>,
    pub sorted_recommended: Vec<String>,
    pub sorted_reviewed: Vec<String>,
    pub search_index: HashMap<String, String>,
}

impl SteamGameStoreIndex {
    pub fn new() -> Self {
        Self {
            games: Arc::new(Mutex::new(HashMap::new())),
            sorted_recommended: Vec::new(),
            sorted_reviewed: Vec::new(),
            search_index: HashMap::new(),
        }
    }

    pub async fn load_games(&mut self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let index: HashMap<String, GameIndexEntry> = serde_json::from_reader(reader)?;

        let mut games = HashMap::new();
        for (appid, entry) in index {
            games.insert(
                appid.clone(),
                SteamGame {
                    appid: appid.clone(),
                    name: entry.name,
                    header_image: entry.header_image,
                    recommendations: entry.recommendations,
                    positive: entry.positive,
                    negative: entry.negative,
                },
            );
        }

        // Create sorted lists
        let mut all_games: Vec<_> = games.values().collect();

        // Sort by recommendations
        all_games.sort_by(|a, b| b.recommendations.cmp(&a.recommendations));
        self.sorted_recommended = all_games.iter().map(|g| g.appid.clone()).collect();

        // Sort by review score (positive ratio)
        all_games.sort_by(|a, b| {
            let score_a = a.positive as f32 / (a.positive + a.negative).max(1) as f32;
            let score_b = b.positive as f32 / (b.positive + b.negative).max(1) as f32;
            score_b.partial_cmp(&score_a).unwrap_or(Ordering::Equal)
        });
        self.sorted_reviewed = all_games.iter().map(|g| g.appid.clone()).collect();

        // Update the games map
        let mut games_map = self.games.lock().await;
        *games_map = games.clone();

        // Create search index
        let mut search_index = HashMap::new();
        for (appid, game) in &games {
            let normalized = normalize_title(&game.name);
            // Only index reasonably long titles
            if normalized.len() >= 5 {
                search_index.insert(normalized, appid.clone());
            }
        }
        self.search_index = search_index;

        Ok(())
    }

    pub async fn get_games(&self, category: &str, page: usize, page_size: usize) -> Vec<SteamGame> {
        let games_map = self.games.lock().await;

        let sorted_list = match category {
            "most_recommended" => &self.sorted_recommended,
            "best_reviewed" => &self.sorted_reviewed,
            _ => return Vec::new(),
        };

        sorted_list
            .iter()
            .skip(page * page_size)
            .take(page_size)
            .filter_map(|appid| games_map.get(appid).cloned())
            .collect()
    }

    pub fn find_best_match_sync(&self, title: &str) -> Option<SteamGame> {
        let normalized = normalize_title(title);
        if normalized.len() < 5 {
            return None;
        }

        let games = self.games.blocking_lock(); // Use blocking_lock to avoid async
        let matcher = SkimMatcherV2::default();
        let mut best_match = None;
        let mut best_score = 0;

        // Exact match
        if let Some(appid) = self.search_index.get(&normalized) {
            if let Some(game) = games.get(appid) {
                return Some(game.clone());
            }
        }

        // Fuzzy
        for (norm_title, appid) in &self.search_index {
            if let Some(score) = matcher.fuzzy_match(norm_title, &normalized) {
                if score > best_score && score > 70 {
                    best_score = score;
                    best_match = games.get(appid).cloned();
                }
            }
        }

        best_match
    }
}

#[derive(Deserialize)]
struct GameIndexEntry {
    name: String,
    header_image: String,
    recommendations: u32,
    positive: u32,
    negative: u32,
}

// Helper function to normalize game titles for comparison
fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "")
        .replace("onlinefix", "")
        .replace("crack", "")
        .trim()
        .to_string()
}
