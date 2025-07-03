use anyhow::Context;
use librqbit::{
    api::LiveStats, AddTorrent, AddTorrentOptions, ManagedTorrent, Session, SessionOptions,
    TorrentStats,
};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    fs,
    io::Cursor,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::async_runtime::spawn_blocking;
use tokio::sync::{Mutex, RwLock};
use unrar::Archive;

use crate::DownloadProgress;

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub game_title: String,
    pub torrent_path: String,
    pub added_at: Instant,
    pub extracted: bool,
    pub extraction_started: bool,
    pub extract_progress: f64,
    pub extraction_error: Option<String>,
    state: DownloadState,
}

#[derive(Debug, Clone)]
enum DownloadState {
    Downloading,
    Extracting,
    Completed,
    Failed,
}

pub struct TorrentManager {
    session: Arc<Session>,
    torrents: RwLock<HashMap<u64, TorrentInfo>>,
    download_dir: PathBuf,
}

impl TorrentManager {
    pub async fn new(download_dir: PathBuf) -> anyhow::Result<Self> {
        let session = Session::new_with_opts(
            download_dir.clone(),
            SessionOptions {
                disable_dht_persistence: true,
                persistence: None,
                ..Default::default()
            },
        )
        .await?;

        Ok(Self {
            session,
            torrents: RwLock::new(HashMap::new()),
            download_dir,
        })
    }

    pub async fn add_torrent_magnet(
        self: Arc<Self>,
        torrent_url: &str,
        game_title: &str,
    ) -> anyhow::Result<Arc<ManagedTorrent>> {
        log::info!("[TORRENT] Adding torrent for game: {}", game_title);
        log::debug!("[TORRENT] Magnet URL: {}", torrent_url);
        let add_torrent = AddTorrent::Url(Cow::Borrowed(torrent_url));

        log::debug!("[TORRENT] Creating torrent handle...");
        let response = self
            .session
            .add_torrent(
                add_torrent,
                Some(AddTorrentOptions {
                    paused: false,
                    ..Default::default()
                }),
            )
            .await?;
        log::debug!("[TORRENT] Converting to managed handle...");
        let handle = response.into_handle().ok_or_else(|| {
            anyhow::anyhow!("Torrent was added as ListOnly and cannot be managed.")
        })?;

        let id = handle.id();

        log::info!("[TORRENT] Torrent added with ID: {}", id);

        let mut torrents = self.torrents.write().await;
        torrents.insert(
            id as u64,
            TorrentInfo {
                game_title: game_title.to_string(),
                torrent_path: "".to_string(),
                added_at: Instant::now(),
                extracted: false,
                extraction_started: false,
                extract_progress: 0.0,
                extraction_error: None,
                state: DownloadState::Downloading,
            },
        );

        log::debug!("[TORRENT] Torrent info stored in manager");

        // Spawn monitoring task using Arc for shared manager
        let manager = self.clone();
        let handle_clone = handle.clone();
        let game_title = game_title.to_string();

        tokio::spawn(async move {
            log::info!("[TORRENT] Starting monitoring task for ID: {}", id);
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let stats = handle_clone.stats();

                if stats.finished {
                    let game_dir = manager.download_dir.join(&game_title);

                    // Mark extraction as started
                    {
                        let mut torrents = manager.torrents.write().await;
                        if let Some(info) = torrents.get_mut(&(id as u64)) {
                            info.extraction_started = true;
                            info.state = DownloadState::Extracting;
                        }
                    }

                    // Extract archives with progress tracking
                    match manager
                        .extract_archives_in_directory(&game_dir, id as u64)
                        .await
                    {
                        Ok(_) => {
                            let mut torrents = manager.torrents.write().await;
                            if let Some(info) = torrents.get_mut(&(id as u64)) {
                                info.extracted = true;
                                info.extract_progress = 1.0;
                                info.extraction_error = None;
                                info.state = DownloadState::Completed;
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to extract archives for {}: {}", game_title, e);
                            let mut torrents = manager.torrents.write().await;
                            if let Some(info) = torrents.get_mut(&(id as u64)) {
                                info.extraction_error = Some(e.to_string());
                                info.state = DownloadState::Failed;
                            }
                        }
                    }

                    break;
                }
            }
        });

        Ok(handle)
    }

    pub async fn add_torrent(
        self: Arc<Self>,
        torrent_path: &str,
        game_title: &str,
    ) -> anyhow::Result<Arc<ManagedTorrent>> {
        let bytes = tokio::fs::read(torrent_path).await?;
        let add_torrent = AddTorrent::TorrentFileBytes(bytes.into());

        let response = self
            .session
            .add_torrent(
                add_torrent,
                Some(AddTorrentOptions {
                    paused: false,
                    ..Default::default()
                }),
            )
            .await?;

        let handle = response.into_handle().ok_or_else(|| {
            anyhow::anyhow!("Torrent was added as ListOnly and cannot be managed.")
        })?;

        let id = handle.id();

        let mut torrents = self.torrents.write().await;
        torrents.insert(
            id as u64,
            TorrentInfo {
                game_title: game_title.to_string(),
                torrent_path: torrent_path.to_string(),
                added_at: Instant::now(),
                extracted: false,
                extraction_started: false,
                extract_progress: 0.0,
                extraction_error: None,
                state: DownloadState::Downloading,
            },
        );

        // Spawn monitoring task using Arc for shared manager
        let manager = self.clone();
        let handle_clone = handle.clone();
        let game_title = game_title.to_string();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let stats = handle_clone.stats();

                if stats.finished {
                    let game_dir = manager.download_dir.join(&game_title);

                    // Mark extraction as started
                    {
                        let mut torrents = manager.torrents.write().await;
                        if let Some(info) = torrents.get_mut(&(id as u64)) {
                            info.extraction_started = true;
                            info.state = DownloadState::Extracting;
                        }
                    }

                    // Extract archives with progress tracking
                    match manager
                        .extract_archives_in_directory(&game_dir, id as u64)
                        .await
                    {
                        Ok(_) => {
                            let mut torrents = manager.torrents.write().await;
                            if let Some(info) = torrents.get_mut(&(id as u64)) {
                                info.extracted = true;
                                info.extract_progress = 1.0;
                                info.extraction_error = None;
                                info.state = DownloadState::Completed;
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to extract archives for {}: {}", game_title, e);
                            let mut torrents = manager.torrents.write().await;
                            if let Some(info) = torrents.get_mut(&(id as u64)) {
                                info.extraction_error = Some(e.to_string());
                                info.state = DownloadState::Failed;
                            }
                        }
                    }

                    break;
                }
            }
        });

        Ok(handle)
    }

    pub async fn remove_torrent(&self, id: u64) -> anyhow::Result<()> {
        self.session
            .delete(librqbit::api::TorrentIdOrHash::Id(id as usize), true)
            .await?;
        let mut torrents = self.torrents.write().await;
        torrents.remove(&id);
        Ok(())
    }

    pub async fn list_torrents(&self) -> Vec<DownloadProgress> {
        let torrents_map = self.torrents.read().await;

        let result = RefCell::new(Vec::new());

        self.session.with_torrents(|iter| {
            for (id, managed) in iter {
                let id = id as u64;
                if let Some(info) = torrents_map.get(&id) {
                    if matches!(
                        info.state,
                        DownloadState::Downloading | DownloadState::Extracting
                    ) {
                        let stats = managed.stats();
                        result
                            .borrow_mut()
                            .push(torrent_stats_to_progress(&stats, info, id));
                    }
                }
            }
        });

        result.into_inner()
    }

    async fn extract_archives_in_directory(
        &self,
        dir: &PathBuf,
        torrent_id: u64,
    ) -> anyhow::Result<()> {
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(()); // Directory doesn't exist yet
            }
            Err(e) => return Err(e.into()),
        };

        // First count all archive files
        let mut archive_files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if entry.metadata().await?.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    match ext.to_lowercase().as_str() {
                        "rar" | "zip" => archive_files.push(path),
                        _ => continue,
                    }
                }
            }
        }

        let total_files = archive_files.len() as f64;
        if total_files == 0.0 {
            return Ok(());
        }

        // Process each archive file and update progress
        for (i, archive_path) in archive_files.into_iter().enumerate() {
            let current_progress = (i as f64) / total_files;

            // Update progress
            {
                let mut torrents = self.torrents.write().await;
                if let Some(info) = torrents.get_mut(&torrent_id) {
                    info.extract_progress = current_progress;
                }
            }

            match archive_path.extension().and_then(|s| s.to_str()) {
                Some("rar") => self.extract_rar(&archive_path, dir).await?,
                Some("zip") => {} /*self.extract_zip(&archive_path, dir).await?*/,
                _ => continue,
            }
        }

        Ok(())
    }

    async fn extract_rar(
        &self,
        archive_path: &PathBuf,
        destination: &PathBuf,
    ) -> anyhow::Result<()> {
        let archive_path = archive_path.clone();
        let destination = destination.clone();

        println!("[DEBUG] Starting RAR extraction");
        println!("[DEBUG] Archive path: {}", archive_path.display());
        println!("[DEBUG] Destination path: {}", destination.display());

        tokio::task::spawn_blocking(move || {
            println!("[DEBUG] Opening archive...");
            let mut archive = Archive::with_password(&archive_path, "online-fix.me")
                .open_for_processing()
                .unwrap();

            while let Some(header) = archive.read_header()? {
                let filename = header.entry().filename.to_path_buf();
                let relative_path = filename.iter().skip(1).collect::<PathBuf>();
                let out_path = destination.join("Extracted").join(relative_path);
                println!(
                    "{} bytes: {}",
                    header.entry().unpacked_size,
                    header.entry().filename.to_string_lossy(),
                );
                archive = if header.entry().is_file() {
                    // Compose full output path
                    println!("Extracting file to dest: {}", out_path.display());

                    // Make sure parent dirs exist
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    // Pass full file path to extract_to
                    header.extract_to(&out_path)?
                } else {
                    header.skip()?
                };
            }
            Ok(())
        })
        .await?
    }

    pub async fn get_torrent_info(&self, id: u64) -> Option<TorrentInfo> {
        self.torrents.read().await.get(&id).cloned()
    }

    pub fn download_dir(&self) -> &PathBuf {
        &self.download_dir
    }
}

fn torrent_stats_to_progress(
    stats: &TorrentStats,
    info: &TorrentInfo,
    id: u64,
) -> DownloadProgress {
    // Base progress is based on bytes downloaded
    let progress = if stats.total_bytes > 0 {
        stats.progress_bytes as f64 / stats.total_bytes as f64
    } else {
        0.0
    };

    // Extract speed and ETA from LiveStats if available
    let (speed_mb_ps, eta, peers_connected, downloaded) = match &stats.live {
        Some(live) => {
            let speed_mb_ps = live.download_speed.mbps;

            let eta = match &live.time_remaining {
                Some(duration) => duration.to_string(),
                None => "N/A".to_string(),
            };

            let peers_connected = live.snapshot.peer_stats.live as u32;
            let downloaded = stats.progress_bytes as i64;

            (speed_mb_ps, eta, peers_connected, downloaded)
        }
        None => (
            0.0,               // No live download speed
            "N/A".to_string(), // No ETA available
            0,                 // No peers
            stats.progress_bytes as i64,
        ),
    };

    DownloadProgress {
        id,
        name: info.game_title.clone(),
        progress,
        speed_mb_ps,
        peers_connected,
        downloaded,
        total_size: stats.total_bytes as i64,
        extract_progress: 0.0, // Still not handled here
        eta,
    }
}
