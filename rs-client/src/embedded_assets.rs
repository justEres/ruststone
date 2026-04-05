use std::io;
use std::path::PathBuf;

/// Mojang launcher metadata endpoint.
const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// The Minecraft version whose client JAR we extract textures from.
const MC_VERSION: &str = "1.8.9";

/// Stamp written after a successful extraction so we don't re-download.
const ASSETS_STAMP: &str = concat!("mojang-", "1.8.9");

pub fn resolve_runtime_assets_root() -> io::Result<PathBuf> {
    let root = rs_utils::ruststone_assets_root();
    ensure_texturepack(&root)?;
    Ok(root)
}

fn texturepack_present(assets_root: &std::path::Path) -> bool {
    let stamp_path = assets_root.join(".ruststone_assets_stamp");
    let Ok(stamp) = std::fs::read_to_string(stamp_path) else {
        return false;
    };
    if stamp.trim() != ASSETS_STAMP {
        return false;
    }
    assets_root.join("texturepack/assets/minecraft").is_dir()
}

fn ensure_texturepack(assets_root: &std::path::Path) -> io::Result<()> {
    if texturepack_present(assets_root) {
        return Ok(());
    }

    let client = reqwest::blocking::Client::new();

    // Step 1: fetch version manifest and locate 1.8.9 metadata URL.
    tracing::info!("Fetching Mojang version manifest...");
    let manifest: serde_json::Value = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .map_err(|e| io_err(format!("version manifest request failed: {e}")))?
        .json()
        .map_err(|e| io_err(format!("parsing version manifest failed: {e}")))?;

    let version_url = manifest["versions"]
        .as_array()
        .and_then(|vs| {
            vs.iter().find(|v| v["id"].as_str() == Some(MC_VERSION))
        })
        .and_then(|v| v["url"].as_str())
        .ok_or_else(|| io_err(format!("version {MC_VERSION} not found in manifest")))?
        .to_owned();

    // Step 2: fetch version JSON and get client JAR URL.
    tracing::info!("Fetching {MC_VERSION} version metadata...");
    let version_meta: serde_json::Value = client
        .get(&version_url)
        .send()
        .map_err(|e| io_err(format!("version metadata request failed: {e}")))?
        .json()
        .map_err(|e| io_err(format!("parsing version metadata failed: {e}")))?;

    let jar_url = version_meta["downloads"]["client"]["url"]
        .as_str()
        .ok_or_else(|| io_err("client JAR URL not found in version metadata".into()))?
        .to_owned();

    // Step 3: download the client JAR (it is a ZIP).
    tracing::info!("Downloading Minecraft {MC_VERSION} client JAR...");
    let jar_bytes = client
        .get(&jar_url)
        .send()
        .map_err(|e| io_err(format!("JAR download failed: {e}")))?
        .bytes()
        .map_err(|e| io_err(format!("reading JAR bytes failed: {e}")))?;

    // Step 4: extract assets/minecraft/* from the JAR into
    //         {assets_root}/texturepack/assets/minecraft/.
    tracing::info!("Extracting textures from client JAR...");
    let texturepack_root = assets_root.join("texturepack");
    std::fs::create_dir_all(&texturepack_root)?;

    let cursor = std::io::Cursor::new(jar_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| io_err(format!("opening client JAR failed: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| io_err(format!("reading JAR entry {i}: {e}")))?;

        let entry_name = entry.name().to_owned();
        let Some(relative) = entry_name.strip_prefix("assets/minecraft/") else {
            continue;
        };

        let out_path = texturepack_root.join("assets/minecraft").join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }

    std::fs::write(assets_root.join(".ruststone_assets_stamp"), ASSETS_STAMP)?;
    tracing::info!("Texturepack extracted to {}", texturepack_root.display());
    Ok(())
}

fn io_err(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}
