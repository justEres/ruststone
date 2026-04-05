use std::collections::HashMap;

use crate::block_textures::Face;

use super::{BlockstateFile, ModelFile};

pub(super) fn blockstate_name_candidates(block_id: u16, base_name: &str, meta: u8) -> Vec<String> {
    let mut out = Vec::with_capacity(6);
    match block_id {
        1 => {
            let name = match meta {
                1 => "granite",
                2 => "smooth_granite",
                3 => "diorite",
                4 => "smooth_diorite",
                5 => "andesite",
                6 => "smooth_andesite",
                _ => "stone",
            };
            out.push(name.to_string());
        }
        3 => {
            let name = match meta {
                1 => "coarse_dirt",
                2 => "podzol",
                _ => "dirt",
            };
            out.push(name.to_string());
        }
        5 => out.push(format!("{}_planks", wood_variant(meta))),
        6 => out.push(format!("{}_sapling", wood_variant(meta))),
        12 => out.push(if (meta & 1) == 1 { "red_sand" } else { "sand" }.to_string()),
        17 => out.push(format!(
            "{}_log",
            match meta & 0x3 {
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                _ => "oak",
            }
        )),
        18 => out.push(format!(
            "{}_leaves",
            match meta & 0x3 {
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                _ => "oak",
            }
        )),
        24 => {
            let name = match meta & 0x3 {
                1 => "chiseled_sandstone",
                2 => "smooth_sandstone",
                _ => "sandstone",
            };
            out.push(name.to_string());
        }
        35 => out.push(format!("{}_wool", color_variant(meta))),
        37 => out.push("dandelion".to_string()),
        38 => {
            let name = match meta {
                1 => "blue_orchid",
                2 => "allium",
                3 => "houstonia",
                4 => "red_tulip",
                5 => "orange_tulip",
                6 => "white_tulip",
                7 => "pink_tulip",
                8 => "oxeye_daisy",
                _ => "poppy",
            };
            out.push(name.to_string());
        }
        43 | 44 => {
            let name = match meta & 0x7 {
                1 => "sandstone_slab",
                2 => "wood_old_slab",
                3 => "cobblestone_slab",
                4 => "brick_slab",
                5 => "stone_brick_slab",
                6 => "nether_brick_slab",
                7 => "quartz_slab",
                _ => "stone_slab",
            };
            out.push(name.to_string());
        }
        95 => out.push(format!("{}_stained_glass", color_variant(meta))),
        97 => {
            let name = match meta {
                1 => "cobblestone",
                2 => "stonebrick",
                3 => "mossy_stonebrick",
                4 => "cracked_stonebrick",
                5 => "chiseled_stonebrick",
                _ => "stone",
            };
            out.push(name.to_string());
        }
        98 => {
            let name = match meta {
                1 => "mossy_stonebrick",
                2 => "cracked_stonebrick",
                3 => "chiseled_stonebrick",
                _ => "stonebrick",
            };
            out.push(name.to_string());
        }
        126 => out.push(format!("{}_slab", wood_variant(meta))),
        159 => out.push(format!("{}_stained_hardened_clay", color_variant(meta))),
        160 => out.push(format!("{}_stained_glass_pane", color_variant(meta))),
        161 => out.push(format!(
            "{}_leaves",
            if (meta & 0x1) == 1 { "dark_oak" } else { "acacia" }
        )),
        162 => out.push(format!(
            "{}_log",
            if (meta & 0x1) == 1 { "dark_oak" } else { "acacia" }
        )),
        171 => out.push(format!("{}_carpet", color_variant(meta))),
        175 => {
            let name = match meta & 0x7 {
                0 => "sunflower",
                1 => "lilac",
                2 => "double_tallgrass",
                3 => "large_fern",
                4 => "rose_bush",
                5 => "peony",
                _ => "sunflower",
            };
            out.push(name.to_string());
        }
        _ => {}
    }
    out.push(base_name.to_string());
    dedup_keep_order(out)
}

pub(super) fn block_item_model_name_candidates(
    block_id: u16,
    base_name: &str,
    meta: u8,
) -> Vec<String> {
    let mut out = blockstate_name_candidates(block_id, base_name, meta);
    match base_name {
        "tallgrass" => {
            out.push("tall_grass".to_string());
            out.push(if (meta & 0x3) == 2 { "fern" } else { "grass" }.to_string());
        }
        "deadbush" => out.push("dead_bush".to_string()),
        "yellow_flower" => out.push("dandelion".to_string()),
        "red_flower" => {
            out.push(
                match meta {
                    1 => "blue_orchid",
                    2 => "allium",
                    3 => "houstonia",
                    4 => "red_tulip",
                    5 => "orange_tulip",
                    6 => "white_tulip",
                    7 => "pink_tulip",
                    8 => "oxeye_daisy",
                    _ => "poppy",
                }
                .to_string(),
            );
        }
        "fence" => out.push("oak_fence".to_string()),
        "fence_gate" => out.push("oak_fence_gate".to_string()),
        "standing_sign" | "wall_sign" => out.push("sign".to_string()),
        "wooden_door" => out.push("oak_door".to_string()),
        "unpowered_repeater" | "powered_repeater" => out.push("repeater".to_string()),
        "unpowered_comparator" | "powered_comparator" => out.push("comparator".to_string()),
        "lit_redstone_lamp" => out.push("redstone_lamp".to_string()),
        "daylight_detector_inverted" => out.push("daylight_detector".to_string()),
        "double_stone_slab" | "double_stone_slab2" => out.push("stone_slab".to_string()),
        "double_wooden_slab" | "wooden_slab" => out.push("oak_slab".to_string()),
        "piston_head" | "piston_extension" => out.push("piston".to_string()),
        "monster_egg" => out.push(
            match meta {
                1 => "cobblestone_monster_egg",
                2 => "stone_brick_monster_egg",
                3 => "mossy_brick_monster_egg",
                4 => "cracked_brick_monster_egg",
                5 => "chiseled_brick_monster_egg",
                _ => "stone_monster_egg",
            }
            .to_string(),
        ),
        "standing_banner" | "wall_banner" => out.push("banner".to_string()),
        _ => {}
    }
    dedup_keep_order(out)
}

pub(super) fn append_png(mut s: String) -> String {
    if !s.ends_with(".png") {
        s.push_str(".png");
    }
    s
}

pub(super) fn split_model_key(key: &str) -> Option<(&str, &str)> {
    let (namespace, path) = key.split_once(':')?;
    Some((namespace, path))
}

pub(super) fn pick_model_name(state: &BlockstateFile) -> Option<String> {
    if let Some(variants) = &state.variants {
        if let Some(entry) = variants.get("") {
            return Some(entry.first_model_name());
        }
        if let Some(entry) = variants.get("normal") {
            return Some(entry.first_model_name());
        }
        if let Some((_k, v)) = variants.iter().next() {
            return Some(v.first_model_name());
        }
    }
    if let Some(multipart) = &state.multipart
        && let Some(part) = multipart.first()
    {
        return Some(part.apply.first_model_name());
    }
    None
}

pub(super) fn lookup_texture_key(model: &ModelFile, key: &str) -> Option<String> {
    model
        .textures
        .as_ref()
        .and_then(|map| map.get(key))
        .cloned()
}

pub(super) fn resolve_texture_ref(model: &ModelFile, tex_ref: &str, depth: usize) -> Option<String> {
    if depth > 16 {
        return None;
    }
    if let Some(key) = tex_ref.strip_prefix('#') {
        let next = lookup_texture_key(model, key)?;
        return resolve_texture_ref(model, &next, depth + 1);
    }
    if let Some(path) = tex_ref.strip_prefix("minecraft:") {
        return Some(path.to_string());
    }
    Some(tex_ref.to_string())
}

pub(super) fn resolve_texture_ref_map(
    textures: &HashMap<String, String>,
    tex_ref: &str,
    depth: usize,
) -> Option<String> {
    if depth > 24 {
        return None;
    }
    if let Some(key) = tex_ref.strip_prefix('#') {
        let next = textures.get(key)?.clone();
        return resolve_texture_ref_map(textures, &next, depth + 1);
    }
    if let Some(path) = tex_ref.strip_prefix("minecraft:") {
        return Some(path.to_string());
    }
    Some(tex_ref.to_string())
}

pub(super) fn template_texture_key(parent: &str, face: Face) -> Option<&'static str> {
    let short = parent
        .strip_prefix("minecraft:")
        .unwrap_or(parent)
        .strip_prefix("block/")
        .unwrap_or(parent);
    let key = match short {
        "cube_all" => "all",
        "cube_bottom_top" => match face {
            Face::PosY => "top",
            Face::NegY => "bottom",
            _ => "side",
        },
        "cube_top" => "top",
        "cube_column" | "cube_column_horizontal" => match face {
            Face::PosY | Face::NegY => "end",
            _ => "side",
        },
        "cube" => match face {
            Face::PosX => "east",
            Face::NegX => "west",
            Face::PosY => "up",
            Face::NegY => "down",
            Face::PosZ => "south",
            Face::NegZ => "north",
        },
        "orientable" | "furnace" | "dispenser" | "dropper" | "command_block" => match face {
            Face::PosY => "top",
            Face::NegY => "bottom",
            _ => "side",
        },
        "orientable_with_bottom" => match face {
            Face::PosY => "top",
            Face::NegY => "bottom",
            _ => "side",
        },
        "cross" | "tinted_cross" => "cross",
        "torch" => "torch",
        "rail_flat" | "rail_raised_ne" | "rail_raised_sw" | "rail_curved" => "rail",
        "slab" | "half_slab" => match face {
            Face::PosY => "top",
            Face::NegY => "bottom",
            _ => "side",
        },
        "stairs" | "inner_stairs" | "outer_stairs" => match face {
            Face::PosY => "top",
            Face::NegY => "bottom",
            _ => "side",
        },
        _ => return None,
    };
    Some(key)
}

pub(super) fn guess_model_texture_ref(model: &ModelFile, face: Face) -> Option<String> {
    let textures = model.textures.as_ref()?;
    let mut keys: Vec<&str> = match face {
        Face::PosY => vec!["up", "top", "end", "all", "side", "particle", "texture"],
        Face::NegY => vec!["down", "bottom", "end", "all", "side", "particle", "texture"],
        Face::PosX | Face::NegX | Face::PosZ | Face::NegZ => vec![
            "side", "front", "all", "north", "south", "east", "west", "texture", "particle",
        ],
    };
    for key in keys.drain(..) {
        if let Some(v) = textures.get(key) {
            return Some(v.clone());
        }
    }
    textures.values().next().cloned()
}

fn wood_variant(meta: u8) -> &'static str {
    match meta & 0x7 {
        1 => "spruce",
        2 => "birch",
        3 => "jungle",
        4 => "acacia",
        5 => "dark_oak",
        _ => "oak",
    }
}

fn color_variant(meta: u8) -> &'static str {
    match meta & 0xF {
        0 => "white",
        1 => "orange",
        2 => "magenta",
        3 => "light_blue",
        4 => "yellow",
        5 => "lime",
        6 => "pink",
        7 => "gray",
        8 => "silver",
        9 => "cyan",
        10 => "purple",
        11 => "blue",
        12 => "brown",
        13 => "green",
        14 => "red",
        _ => "black",
    }
}

fn dedup_keep_order(input: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(input.len());
    for entry in input {
        if !out.iter().any(|v| v == &entry) {
            out.push(entry);
        }
    }
    out
}
