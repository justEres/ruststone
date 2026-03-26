use std::path::{Path, PathBuf};

pub const RUSTSTONE_ASSETS_ROOT_ENV: &str = "RUSTSTONE_ASSETS_ROOT";

pub fn ruststone_assets_root() -> PathBuf {
    if let Ok(explicit) = std::env::var(RUSTSTONE_ASSETS_ROOT_ENV) {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return path;
        }
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let sibling_assets = exe_dir.join("assets");
        if sibling_assets.exists() {
            return sibling_assets;
        }
    }

    let repo_assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs-client/assets");
    if repo_assets.exists() {
        return repo_assets;
    }

    PathBuf::from("assets")
}

pub fn texturepack_minecraft_root() -> PathBuf {
    ruststone_assets_root().join("texturepack/assets/minecraft")
}

pub fn texturepack_textures_root() -> PathBuf {
    texturepack_minecraft_root().join("textures")
}

pub fn sound_cache_root() -> PathBuf {
    PathBuf::from("ruststone_sound_cache")
}

pub fn sound_cache_minecraft_root() -> PathBuf {
    sound_cache_root().join("assets/minecraft")
}
