use reqwest::{blocking::get, blocking::Client};
use scraper::{Html, Selector};
use serde::Serialize;
use std::error::Error;

use crate::STEAM_GAME_STORE_INDEX;

#[derive(Serialize)]
pub struct Game {
    pub title: String,
    pub link: String,
    pub image: String,
    pub release_date: Option<String>,
    pub modes: Option<String>,
    pub views: Option<String>,
}

// Scrape the main page for games
pub fn scrape_games(page: usize) -> Result<Vec<Game>, Box<dyn Error>> {
    let url = if page > 1 {
        format!("https://online-fix.me/page/{}/", page)
    } else {
        "https://online-fix.me".to_string()
    };
    let client = Client::new();

    // Fetch the HTML content with headers
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .header("Referer", "https://online-fix.me")
        .send()?
        .text()?;

    let document = Html::parse_document(&response);
    let article_selector = Selector::parse("article.news").unwrap(); // Select articles
    let title_selector = Selector::parse("h2.title").unwrap();
    let link_selector = Selector::parse("a.big-link").unwrap();
    let img_selector = Selector::parse(".image a.img img").unwrap();
    let release_selector = Selector::parse(".preview-text").unwrap();
    let inform_panel_selector = Selector::parse(".inform-panel").unwrap();

    let mut games = Vec::new();

    for article in document.select(&article_selector) {
        // Title and link
        let title_element = article.select(&title_selector).next();
        let link_element = article.select(&link_selector).next();

        let title = title_element
            .and_then(|e| Some(e.text().collect::<Vec<_>>().join("").trim().to_string()))
            .unwrap_or_default();

        // Remove the last 7 characters (safely for UTF-8)
        let title = if title.chars().count() > 8 {
            title
                .chars()
                .take(title.chars().count() - 8)
                .collect::<String>()
        } else {
            title
        };

        let link = link_element
            .and_then(|e| e.value().attr("href"))
            .unwrap_or("")
            .to_string();

        // Image
        let image = article
            .select(&img_selector)
            .next()
            .and_then(|e| e.value().attr("data-src")) // Use "data-src" for lazy-loaded images
            .unwrap_or("")
            .to_string();

        // Release date and game modes
        let preview_text = article
            .select(&release_selector)
            .next()
            .and_then(|e| Some(e.inner_html()))
            .unwrap_or_default();

        let release_date = preview_text
            .split("<br />")
            .find(|line| line.contains("Релиз игры:"))
            .map(|line| line.replace("<b>Релиз игры:</b>", "").trim().to_string());

        let modes = preview_text
            .split("<br />")
            .find(|line| line.contains("Режимы:"))
            .map(|line| line.replace("<b>Режимы:</b>", "").trim().to_string());

        // Views (from inform-panel)
        let views = article
            .select(&inform_panel_selector)
            .next()
            .and_then(|e| e.text().find(|line| line.contains("fa-eye")))
            .map(|line| line.trim().to_string());

        // Push to list
        games.push(Game {
            title,
            link,
            image,
            release_date,
            modes,
            views,
        });
    }

    Ok(games)
}
