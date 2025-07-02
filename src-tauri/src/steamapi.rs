use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use strsim::jaro_winkler;

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

pub async fn fetch_game_details(appid: u32) -> Result<GameDetails, Box<dyn std::error::Error>> {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={}", appid);
    let response = reqwest::get(&url).await?;
    let response_json: serde_json::Value = response.json().await?;

    // Print the raw JSON response for debugging

    // Parse the response for the specific appid
    if response_json[appid.to_string()]["success"].as_bool().unwrap_or(false) {
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

/// Asynchronously load Steam games into the global store
pub async fn load_steam_games(file_path: &str) -> Result<Arc<Mutex<SteamGameStore>>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    // Deserialize the JSON directly into a Vec<SteamApp>
    let mut games: Vec<SteamApp> = serde_json::from_reader(reader)?;

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
