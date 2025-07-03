use reqwest::header;
use reqwest::Client;
use scraper::{Html, Selector};
use std::{collections::HashMap, error::Error};

use crate::TORRENT_MANAGER;

pub struct AuthenticatedClient {
    client: Client,
    cookie_store: HashMap<String, String>,
}

impl AuthenticatedClient {
    pub async fn new(cf_clearance: &str, php_sessid: &str) -> Result<Self, Box<dyn Error>> {
        // Use a custom reqwest client builder with HTTP/2 and Rustls
        let client = reqwest::Client::builder().build()?;

        // Initialize with provided cookies
        let mut cookie_store = HashMap::new();
        cookie_store.insert("cf_clearance".to_string(), cf_clearance.to_string());
        cookie_store.insert("PHPSESSID".to_string(), php_sessid.to_string());
        cookie_store.insert("dle_user_id".to_string(), "1956012".to_string());
        cookie_store.insert(
            "dle_password".to_string(),
            "5635960d1de3680613ba0cab35bfcf56".to_string(),
        );
        cookie_store.insert("u_e7f652f7be".to_string(), "1".to_string());
        cookie_store.insert("e7f652f7be_delayCount".to_string(), "12".to_string());

        Ok(Self {
            client,
            cookie_store,
        })
    }

    /// Construct a single `Cookie` header from the stored cookies
    fn construct_cookie_header(&self) -> String {
        let cookie_header = self
            .cookie_store
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("; ");
        cookie_header
    }

    /// Mimics the initial request to get the `online_fix_auth` cookie
    pub async fn fetch_online_fix_auth(&mut self, game_title: &str) -> Result<(), Box<dyn Error>> {
        // Construct the game-specific URL
        let game_url = format!(
            "https://uploads.online-fix.me:2053/torrents/{}/",
            game_title.replace(" ", "%20")
        );

        // Construct the headers and cookies
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0",
            ),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            header::ACCEPT_LANGUAGE,
            header::HeaderValue::from_static("en-US,en;q=0.5"),
        );
        headers.insert(
            header::ACCEPT_ENCODING,
            header::HeaderValue::from_static("gzip, deflate, br, zstd"),
        );
        headers.insert(
            header::REFERER,
            header::HeaderValue::from_static("https://online-fix.me/"),
        );

        // Add cookies as separate headers
        for (cookie_name, cookie_value) in &self.cookie_store {
            let cookie_header = format!("{}={}", cookie_name, cookie_value);
            headers.insert(
                header::COOKIE,
                header::HeaderValue::from_str(&cookie_header)?,
            );
        }

        // Send the request
        let response = self.client.get(&game_url).headers(headers).send().await;

        // Handle request errors
        if let Err(err) = response {
            return Err(format!("Failed to send request: {:?}", err).into());
        }

        let response = response.unwrap();

        // Clone headers and status before consuming the response
        let headers = response.headers().clone();
        let status = response.status();

        // Buffer the response body
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to fetch body".to_string());

        // Check for successful status code
        if !status.is_success() {
            println!("Response body for error: {}", body);
            return Err(format!("Request failed with status: {}. Body: {}", status, body).into());
        }

        // Extract the `Set-Cookie` header from cloned headers
        if let Some(set_cookie) = headers.get(header::SET_COOKIE) {
            let set_cookie_str = set_cookie.to_str().unwrap_or("");

            // Extract `online_fix_auth`
            if let Some(auth_cookie) = set_cookie_str
                .split(";")
                .find(|s| s.contains("online_fix_auth="))
            {
                let auth_value = auth_cookie
                    .trim_start_matches("online_fix_auth=")
                    .to_string();
                self.cookie_store
                    .insert("online_fix_auth".to_string(), auth_value.clone());
                Ok(())
            } else {
                Err("Failed to fetch `online_fix_auth` cookie".into())
            }
        } else {
            Err("No `Set-Cookie` header in response".into())
        }
    }

    /// Use the `online_fix_auth` cookie to download the torrent file
    pub async fn download_torrent(&self, game_title: &str) -> Result<(), Box<dyn Error>> {
        // Construct the game-specific URL
        let game_url = format!(
            "https://uploads.online-fix.me:2053/torrents/{}/",
            game_title.replace(" ", "%20")
        );

        // Fetch HTML to find the torrent file link
        let html = self.fetch_html(&game_url).await?;
        let torrent_file_name = self.parse_torrent_file_name(&html)?;
        let torrent_url = format!("{}/{}", game_url, torrent_file_name);

        // Determine the default torrents directory
        let torrents_dir = if cfg!(target_os = "windows") {
            format!("{}/PirateLand/torrents", std::env::var("APPDATA")?)
        } else {
            format!("{}/.pirateland/torrents", std::env::var("HOME")?)
        };

        // Ensure the directory exists
        std::fs::create_dir_all(&torrents_dir)?;

        // Path for the downloaded torrent file
        let output_path = format!("{}/{}", torrents_dir, torrent_file_name);

        // Download the torrent file
        let response = self
            .client
            .get(&torrent_url)
            .header("Cookie", self.construct_cookie_header())
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0",
            )
            .send()
            .await?;

        if response.status().is_success() {
            let content = response.bytes().await?;
            std::fs::write(&output_path, &content)?;
            println!("Torrent downloaded to {}", output_path);

            // ✅ Add torrent directly instead of sending to Go server
            // Get manager clone without holding lock
            let manager_clone = {
                let manager = TORRENT_MANAGER.lock().await;
                if let Some(m) = manager.as_ref() {
                    m.clone()
                } else {
                    return Err("Torrent manager not initialized".into());
                }
            };

            // Use clone to add torrent
            manager_clone
                .add_torrent(&output_path, game_title)
                .await
                .map_err(|e| format!("Failed to add torrent: {}", e))?;
            Ok(())
        } else {
            Err(format!("Failed to download torrent: {}", response.status()).into())
        }
    }

    /// Notify the Go server about the downloaded torrent
    pub async fn drop_torrent(
        &self,
        torrent_file_path: &str,
        game_title: &str,
    ) -> Result<(), Box<dyn Error>> {
        let go_server_url = "http://localhost:8091/drop-torrent";

        let response = self
            .client
            .post(go_server_url)
            .json(&serde_json::json!({ "file_path": torrent_file_path}))
            .send()
            .await?;

        if response.status().is_success() {
            println!("Successfully notified Go server about torrent file.");
            Ok(())
        } else {
            Err(format!("Failed to notify Go server: {}", response.status()).into())
        }
    }

    /// Fetch the HTML of the given URL
    async fn fetch_html(&self, url: &str) -> Result<String, Box<dyn Error>> {
        let response = self
            .client
            .get(url)
            .header("Cookie", self.construct_cookie_header())
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0",
            )
            .send()
            .await?;

        if response.status().is_success() {
            let html = response.text().await?;
            Ok(html)
        } else {
            Err(format!("Failed to fetch HTML: {}", response.status()).into())
        }
    }

    /// Parse the torrent file name from HTML
    fn parse_torrent_file_name(&self, html: &str) -> Result<String, Box<dyn Error>> {
        let document = Html::parse_document(html);
        let link_selector = Selector::parse("a").unwrap();

        if let Some(element) = document.select(&link_selector).find(|e| {
            let href = e.value().attr("href").unwrap_or("");
            href.ends_with(".torrent")
        }) {
            let file_name = element.value().attr("href").unwrap_or("").to_string();
            Ok(file_name)
        } else {
            println!("No torrent file link found in HTML.");
            Err("No torrent file link found".into())
        }
    }
}
