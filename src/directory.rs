use crate::{TORRENTS, torrent::Torrent};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub async fn prepare_torrent_folder(directory: PathBuf) {
    if !std::path::Path::new(&directory).is_dir() {
        tokio::fs::create_dir_all(directory.clone())
            .await
            .unwrap_or_else(|_e| {
                error!("Cannot create torrent folder directory(ies)");
            });
        info!("Torrent directory created: {}", directory.display());
    }
    info!("Will load torrents from: {}", directory.display());
}

/// Load torrents from the provided directory.
///
/// Add a torrent to the list. If the filename does not end with .torrent, the file is not processed.
pub async fn load_torrents(directory: PathBuf) -> usize {
    let paths = std::fs::read_dir(&directory).expect("Cannot read torrent directory");
    let mut count = 0usize;
    for p in paths {
        let path = p.expect("Cannot get torrent path").path();
        if let Some(extension) = path.extension()
            && extension.eq_ignore_ascii_case("torrent")
        {
            match Torrent::from_file(path.clone()) {
                Ok(torrent) => {
                    info!("Found torrent {}", path.display());
                    if torrent.urls.is_empty() {
                        warn!(
                            "Skipping torrent because there is no URL (DHT or not supported URLs)"
                        );
                        continue;
                    }
                    if TORRENTS.contains_key(&torrent.info_hash) {
                        warn!("A torrent with the same hash is already added");
                    } else {
                        let info_hash = torrent.info_hash;
                        TORRENTS.insert(info_hash, Arc::new(Mutex::new(torrent)));
                        count += 1;
                    }
                }
                Err(e) => error!("Cannot add torrent {}: {e}", path.display()),
            }
        }
    }
    info!("{} torrent(s) loaded", count);
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Return the path to a real .torrent fixture bundled with the test suite.
    fn ubuntu_fixture() -> &'static str {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/ubuntu-20.04.4-desktop-amd64.iso.torrent"
        )
    }

    #[tokio::test]
    async fn test_load_torrents_counts_valid_files() {
        crate::TORRENTS.clear();
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(ubuntu_fixture(), dir.path().join("ubuntu.torrent")).unwrap();

        let count = load_torrents(dir.path().to_path_buf()).await;
        assert_eq!(count, 1);
        crate::TORRENTS.clear();
    }

    #[tokio::test]
    async fn test_load_torrents_skips_non_torrent_files() {
        crate::TORRENTS.clear();
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("README.txt"), b"not a torrent")
            .await
            .unwrap();

        let count = load_torrents(dir.path().to_path_buf()).await;
        assert_eq!(count, 0);
        crate::TORRENTS.clear();
    }

    #[tokio::test]
    async fn test_load_torrents_skips_duplicates() {
        crate::TORRENTS.clear();
        let dir = tempfile::tempdir().unwrap();
        // Two copies of the same torrent → same info hash → only one inserted
        std::fs::copy(ubuntu_fixture(), dir.path().join("ubuntu1.torrent")).unwrap();
        std::fs::copy(ubuntu_fixture(), dir.path().join("ubuntu2.torrent")).unwrap();

        let count = load_torrents(dir.path().to_path_buf()).await;
        assert_eq!(count, 1);
        crate::TORRENTS.clear();
    }

    #[tokio::test]
    async fn test_load_torrents_empty_directory() {
        crate::TORRENTS.clear();
        let dir = tempfile::tempdir().unwrap();
        let count = load_torrents(dir.path().to_path_buf()).await;
        assert_eq!(count, 0);
        crate::TORRENTS.clear();
    }
}
