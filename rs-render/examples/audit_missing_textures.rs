use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rs_render::{ModelFace as Face, build_block_texture_mapping};
use rs_render::default_model_roots;
use rs_utils::{block_registry_key, ruststone_assets_root};

const TEXTURE_BASE: &str = "texturepack/assets/minecraft/textures/blocks/";

fn main() {
    let name_to_index = collect_texture_name_map();
    let mut resolver = rs_render::BlockModelResolver::new(default_model_roots());
    let mapping = build_block_texture_mapping(&name_to_index, Some(&mut resolver));

    let mut fully_missing = Vec::new();
    let mut partial_missing = Vec::new();
    let stone_index = mapping.texture_index_by_name("stone.png").unwrap_or(mapping.missing_index);
    let mut all_stone_meta0 = Vec::new();

    for block_id in 0u16..=4095u16 {
        let Some(registry_key) = block_registry_key(block_id) else {
            continue;
        };
        let mut meta0_all_stone = true;
        let state0 = block_id << 4;
        for face in [
            Face::PosX,
            Face::NegX,
            Face::PosY,
            Face::NegY,
            Face::PosZ,
            Face::NegZ,
        ] {
            if mapping.texture_index_for_state(state0, face) != stone_index {
                meta0_all_stone = false;
                break;
            }
        }
        if meta0_all_stone {
            all_stone_meta0.push((block_id, registry_key.to_string()));
        }
        let mut missing_metas = Vec::new();
        for meta in 0u16..=15u16 {
            let state = (block_id << 4) | meta;
            let mut face_missing = 0u8;
            for face in [
                Face::PosX,
                Face::NegX,
                Face::PosY,
                Face::NegY,
                Face::PosZ,
                Face::NegZ,
            ] {
                if mapping.texture_index_for_state(state, face) == mapping.missing_index {
                    face_missing += 1;
                }
            }
            if face_missing == 6 {
                missing_metas.push(meta as u8);
            }
        }
        if missing_metas.len() == 16 {
            fully_missing.push((block_id, registry_key.to_string()));
        } else if !missing_metas.is_empty() {
            partial_missing.push((block_id, registry_key.to_string(), missing_metas));
        }
    }

    println!("Registered block ids with all metas fully missing:");
    for (id, key) in &fully_missing {
        println!("  id={id:>4} key={key}");
    }
    println!(
        "Total fully missing registered ids: {}",
        fully_missing.len()
    );

    println!("\nRegistered block ids with only some metas fully missing:");
    for (id, key, metas) in &partial_missing {
        println!("  id={id:>4} key={key} metas={metas:?}");
    }
    println!(
        "Total partially missing registered ids: {}",
        partial_missing.len()
    );
    println!("\nRegistered block ids with meta=0 resolving all faces to stone.png:");
    for (id, key) in &all_stone_meta0 {
        println!("  id={id:>4} key={key}");
    }
    println!("Total all-stone(meta0) registered ids: {}", all_stone_meta0.len());
}

fn collect_texture_name_map() -> HashMap<String, u16> {
    let textures_root: PathBuf = ruststone_assets_root().join(TEXTURE_BASE);
    let mut names = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&textures_root) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                && let Some(name) = path.file_name().and_then(|s| s.to_str())
            {
                names.push(name.to_string());
            }
        }
    }
    names.extend(
        [
            "barrier_item.png",
            "chest_normal.png",
            "chest_trapped.png",
            "chest_ender.png",
            "sign_entity.png",
            "head_player_top.png",
            "head_player_bottom.png",
            "head_player_front.png",
            "head_player_back.png",
            "head_player_left.png",
            "head_player_right.png",
        ]
        .into_iter()
        .map(str::to_string),
    );
    names.sort();
    names.dedup();
    if !names.iter().any(|n| n == "missing_texture.png") {
        names.insert(0, "missing_texture.png".to_string());
    }

    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| (name, i as u16))
        .collect()
}
