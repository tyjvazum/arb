// Based on the work by shesek in: https://github.com/casey/ord/pull/1805
use {
    super::Error,
    anyhow::Context,
    bitcoin::hashes::{
        hex::ToHex,
        sha256,
        Hash,
    },
    lava_torrent::{
        bencode::BencodeElem,
        torrent::v1::TorrentBuilder,
    },
    std::{
        ffi::OsString,
        fs,
        path::{
            Path,
            PathBuf,
        },
    },
    urlencoding::encode,
};

// Bittorrent piece length (1MB)
const PIECE_LENGTH: i64 = 1048576;

// Default tracker URIs (included in .torrent & magnet links)
// Note: support multiple, add wss tracker
pub const DEFAULT_TRACKER: &str = "udp://tracker.openbittorrent.com:6969";

// Bootstrap peers for DHT discovery (included in .torrent & magnet links)
pub const DEFAULT_PEER: &str = "dht.aelitis.com:6881";

pub(crate) fn make_offchain_inscription(
    file_path: impl AsRef<Path>,
    torrent_path: Option<impl AsRef<Path>>,
    tracker_url: &str,
    peer_addr: &str,
) -> Result<[String; 2], Error> {
    // TorrentBuilder requires absolute paths
    let file_path = fs::canonicalize(file_path)?;

    // Create the torrent and gets its infohash
    let torrent = TorrentBuilder::new(&file_path, PIECE_LENGTH)
        .set_announce(Some(tracker_url.to_string()))
        .add_extra_field("nodes".to_string(), bencode_nodes(peer_addr))
        .build()
        .with_context(|| "TorrentBuilder failed")?;
    let infohash = torrent.info_hash_bytes();

    // Write the .torrent file (by default, to <path>.torrent)
    let torrent_path = get_torrent_path(&file_path, torrent_path);
    log::info!(
        "Writing torrent with infohash {} to {}",
        infohash.to_hex(),
        torrent_path.display()
    );
    torrent
        .write_into_file(torrent_path)
        .with_context(|| "failed writing .torrent file")?;

    // Calculate the file's SHA256
    // Note: streaming hash to avoid loading the entire file in memory
    let contents = fs::read(&file_path)
        .with_context(|| format!("io error reading {}", file_path.display()))?;
    let sha256hash = sha256::Hash::hash(&contents).into_inner().to_vec();

    Ok([
        format!(
            "magnet:?xt=urn:btih:{}&tr={}&x.pe={}",
            infohash.to_hex(),
            encode(DEFAULT_TRACKER),
            encode(DEFAULT_PEER),
        ),
        sha256hash.to_hex(),
    ])
}

fn get_torrent_path(
    file_path: &Path,
    torrent_path: Option<impl AsRef<Path>>,
) -> PathBuf {
    if let Some(torrent_path) = torrent_path {
        torrent_path.as_ref().to_owned()
    } else {
        let mut fileoss: OsString = file_path.into();
        fileoss.push(".torrent");
        fileoss.into()
    }
}

fn bencode_nodes(nodes: &str) -> BencodeElem {
    BencodeElem::List(
        nodes
            .trim()
            .split(' ')
            .filter_map(|node| {
                let mut parts = node.split(':'); // host:port
                Some(BencodeElem::List(vec![
                    parts.next()?.into(),
                    parts.next()?.parse::<i64>().ok()?.into(),
                ]))
            })
            .collect(),
    )
}
