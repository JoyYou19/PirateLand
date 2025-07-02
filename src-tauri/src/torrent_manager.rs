use librqbit::{
    api::LiveStats, AddTorrent, AddTorrentOptions, ManagedTorrent, Session, SessionOptions,
    TorrentStats,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};

use crate::DownloadProgress;

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub game_title: String,
    pub torrent_path: String,
    pub added_at: Instant,
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

    pub async fn add_torrent(
        &self,
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
            },
        );

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
                    let stats = managed.stats();
                    result
                        .borrow_mut()
                        .push(torrent_stats_to_progress(&stats, info, id));
                }
            }
        });

        result.into_inner()
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
            let speed_mb_ps = live.download_speed.mbps / 8.0;

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
