use std::io;
use std::path::PathBuf;

#[cfg(feature = "bundle_assets")]
use std::path::Path;

#[cfg(feature = "bundle_assets")]
use include_dir::{Dir, include_dir};

#[cfg(feature = "bundle_assets")]
static EMBEDDED_ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets");

#[cfg(feature = "bundle_assets")]
const EMBEDDED_STAMP: &str = concat!("ruststone-", env!("CARGO_PKG_VERSION"));

#[cfg(feature = "bundle_assets")]
fn should_extract_assets(target_root: &Path) -> bool {
    let stamp_path = target_root.join(".ruststone_embedded_stamp");
    let Ok(stamp) = std::fs::read_to_string(stamp_path) else {
        return true;
    };
    if stamp.trim() != EMBEDDED_STAMP {
        return true;
    }
    !target_root.join("texturepack/assets/minecraft").is_dir()
}

#[cfg(feature = "bundle_assets")]
pub fn resolve_runtime_assets_root() -> io::Result<PathBuf> {
    let target_root = std::env::temp_dir().join(format!("ruststone-assets-{}", EMBEDDED_STAMP));

    if should_extract_assets(&target_root) {
        std::fs::create_dir_all(&target_root)?;
        EMBEDDED_ASSETS.extract(&target_root)?;
        std::fs::write(
            target_root.join(".ruststone_embedded_stamp"),
            EMBEDDED_STAMP.as_bytes(),
        )?;
    }

    Ok(target_root)
}

#[cfg(not(feature = "bundle_assets"))]
pub fn resolve_runtime_assets_root() -> io::Result<PathBuf> {
    Ok(rs_utils::ruststone_assets_root())
}
