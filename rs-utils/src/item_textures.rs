// Item texture candidate mapping for Minecraft 1.8.9-style texture packs.
//
// This is intentionally heuristic-based (not the full JSON model pipeline). It aims to cover
// common survival items/blocks and metadata variants well enough for UI icons and basic in-world
// item sprites.

use crate::registry::{block_registry_key, item_registry_key};

/// Returns prioritized texture path candidates relative to `assets/minecraft/textures/`.
pub fn item_texture_candidates(item_id: i32, damage: i16) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(10);

    // Keep important subtype-aware item mappings explicit.
    match item_id {
        // Planks
        5 => {
            let planks = match damage {
                1 => "planks_spruce",
                2 => "planks_birch",
                3 => "planks_jungle",
                4 => "planks_acacia",
                5 => "planks_big_oak",
                _ => "planks_oak",
            };
            push_candidate(&mut out, format!("blocks/{planks}.png"));
        }
        // Logs
        17 => {
            let log = match damage & 0x3 {
                1 => "log_spruce",
                2 => "log_birch",
                3 => "log_jungle",
                _ => "log_oak",
            };
            push_candidate(&mut out, format!("blocks/{log}.png"));
        }
        162 => {
            let log = match damage & 0x3 {
                1 => "log_big_oak",
                _ => "log_acacia",
            };
            push_candidate(&mut out, format!("blocks/{log}.png"));
        }
        // Leaves
        18 => {
            let leaves = match damage & 0x3 {
                1 => "leaves_spruce",
                2 => "leaves_birch",
                3 => "leaves_jungle",
                _ => "leaves_oak",
            };
            push_candidate(&mut out, format!("blocks/{leaves}.png"));
        }
        161 => {
            let leaves = match damage & 0x3 {
                1 => "leaves_big_oak",
                _ => "leaves_acacia",
            };
            push_candidate(&mut out, format!("blocks/{leaves}.png"));
        }
        // Saplings
        6 => {
            let sapling = match damage & 0x7 {
                1 => "sapling_spruce",
                2 => "sapling_birch",
                3 => "sapling_jungle",
                4 => "sapling_acacia",
                5 => "sapling_roofed_oak",
                _ => "sapling_oak",
            };
            push_candidate(&mut out, format!("blocks/{sapling}.png"));
        }
        // Sand variants
        12 => {
            let sand = if (damage & 0x1) != 0 {
                "red_sand"
            } else {
                "sand"
            };
            push_candidate(&mut out, format!("blocks/{sand}.png"));
        }
        // Dirt variants
        3 => {
            let dirt = match damage & 0x3 {
                1 => "coarse_dirt",
                2 => "dirt_podzol_top",
                _ => "dirt",
            };
            push_candidate(&mut out, format!("blocks/{dirt}.png"));
        }
        // Stone variants (very common to appear as items with meta)
        1 => {
            let stone = match damage {
                1 => "stone_granite",
                2 => "stone_granite_smooth",
                3 => "stone_diorite",
                4 => "stone_diorite_smooth",
                5 => "stone_andesite",
                6 => "stone_andesite_smooth",
                _ => "stone",
            };
            push_candidate(&mut out, format!("blocks/{stone}.png"));
        }
        // Wool colors
        35 => {
            let color = wool_color_name(damage);
            push_candidate(&mut out, format!("blocks/wool_colored_{color}.png"));
        }
        // Stained glass (block icon in blocks/)
        95 => {
            let color = wool_color_name(damage);
            push_candidate(&mut out, format!("blocks/glass_{color}.png"));
        }
        // Stained clay
        159 => {
            let color = wool_color_name(damage);
            push_candidate(
                &mut out,
                format!("blocks/hardened_clay_stained_{color}.png"),
            );
        }
        // Dyes
        351 => {
            let color = dye_color_name(damage);
            push_candidate(&mut out, format!("items/dye_powder_{color}.png"));
        }
        // Coal/charcoal
        263 => {
            if damage == 1 {
                push_candidate(&mut out, "items/charcoal.png".to_string());
                push_candidate(&mut out, "items/coal.png".to_string());
            } else {
                push_candidate(&mut out, "items/coal.png".to_string());
                push_candidate(&mut out, "items/charcoal.png".to_string());
            }
        }
        // Fish variants (1.8 has multiple naming conventions across packs)
        349 => {
            // 0=cod, 1=salmon, 2=clownfish, 3=pufferfish
            match damage & 0x3 {
                1 => push_candidate(&mut out, "items/fish_salmon_raw.png".to_string()),
                2 => push_candidate(&mut out, "items/fish_clownfish_raw.png".to_string()),
                3 => push_candidate(&mut out, "items/fish_pufferfish_raw.png".to_string()),
                _ => push_candidate(&mut out, "items/fish_cod_raw.png".to_string()),
            }
            push_candidate(&mut out, "items/fish_raw.png".to_string());
        }
        350 => {
            // 0=cod, 1=salmon
            if (damage & 0x1) != 0 {
                push_candidate(&mut out, "items/fish_salmon_cooked.png".to_string());
            } else {
                push_candidate(&mut out, "items/fish_cod_cooked.png".to_string());
            }
            push_candidate(&mut out, "items/fish_cooked.png".to_string());
        }
        // Potions: same item id, damage indicates drinkable vs splash.
        373 => {
            let splash = (damage & 0x4000) != 0;
            if splash {
                push_candidate(&mut out, "items/potion_bottle_splash.png".to_string());
            } else {
                push_candidate(&mut out, "items/potion_bottle_drinkable.png".to_string());
            }
            // Some packs use legacy name.
            push_candidate(&mut out, "items/potion.png".to_string());
        }
        // Skulls
        397 => {
            let skull = match damage {
                2 => "items/skull_zombie.png",
                3 => "items/skull_char.png",
                4 => "items/skull_creeper.png",
                1 => "items/skull_wither.png",
                _ => "items/skull_skeleton.png",
            };
            push_candidate(&mut out, skull.to_string());
        }
        // Flowers (red flower has meta variants in 1.8)
        38 => {
            let flower = match damage {
                1 => "flower_blue_orchid",
                2 => "flower_allium",
                3 => "flower_houstonia",
                4 => "flower_red_tulip",
                5 => "flower_orange_tulip",
                6 => "flower_white_tulip",
                7 => "flower_pink_tulip",
                8 => "flower_oxeye_daisy",
                _ => "flower_rose",
            };
            push_candidate(&mut out, format!("blocks/{flower}.png"));
        }
        _ => {}
    }

    // General mapping from baked 1.8.9 registries.
    if let Some(key) = item_registry_key(item_id) {
        add_key_candidates(&mut out, key);
    }
    // Block item IDs not present in item registry (1.8 uses numeric IDs for blocks as items).
    if let Some(block_key) = block_registry_key(item_id as u16) {
        add_key_candidates(&mut out, block_key);
    }

    out
}

fn add_key_candidates(out: &mut Vec<String>, key: &str) {
    push_candidate(out, format!("items/{key}.png"));
    push_candidate(out, format!("blocks/{key}.png"));

    // Common naming differences between registry keys and 1.8 texture filenames.
    match key {
        "bow" => push_candidate(out, "items/bow_standby.png".to_string()),
        "fishing_rod" => push_candidate(out, "items/fishing_rod_uncast.png".to_string()),
        "bucket" => push_candidate(out, "items/bucket_empty.png".to_string()),
        "water_bucket" => push_candidate(out, "items/bucket_water.png".to_string()),
        "lava_bucket" => push_candidate(out, "items/bucket_lava.png".to_string()),
        "milk_bucket" => push_candidate(out, "items/bucket_milk.png".to_string()),
        "minecart" => push_candidate(out, "items/minecart_normal.png".to_string()),
        "chest_minecart" => push_candidate(out, "items/minecart_chest.png".to_string()),
        "furnace_minecart" => push_candidate(out, "items/minecart_furnace.png".to_string()),
        "tnt_minecart" => push_candidate(out, "items/minecart_tnt.png".to_string()),
        "hopper_minecart" => push_candidate(out, "items/minecart_hopper.png".to_string()),
        "command_block_minecart" => {
            push_candidate(out, "items/minecart_command_block.png".to_string())
        }
        "wooden_door" => push_candidate(out, "items/door_wood.png".to_string()),
        "iron_door" => push_candidate(out, "items/door_iron.png".to_string()),
        "spruce_door" => push_candidate(out, "items/door_spruce.png".to_string()),
        "birch_door" => push_candidate(out, "items/door_birch.png".to_string()),
        "jungle_door" => push_candidate(out, "items/door_jungle.png".to_string()),
        "acacia_door" => push_candidate(out, "items/door_acacia.png".to_string()),
        "dark_oak_door" => push_candidate(out, "items/door_dark_oak.png".to_string()),
        "wheat_seeds" => push_candidate(out, "items/seeds_wheat.png".to_string()),
        "pumpkin_seeds" => push_candidate(out, "items/seeds_pumpkin.png".to_string()),
        "melon_seeds" => push_candidate(out, "items/seeds_melon.png".to_string()),
        "redstone" => push_candidate(out, "items/redstone_dust.png".to_string()),
        "porkchop" => push_candidate(out, "items/porkchop_raw.png".to_string()),
        "beef" => push_candidate(out, "items/beef_raw.png".to_string()),
        "chicken" => push_candidate(out, "items/chicken_raw.png".to_string()),
        "speckled_melon" => push_candidate(out, "items/melon_speckled.png".to_string()),
        "enchanted_book" => push_candidate(out, "items/book_enchanted.png".to_string()),
        "writable_book" => push_candidate(out, "items/book_writable.png".to_string()),
        "written_book" => push_candidate(out, "items/book_written.png".to_string()),
        "book" => push_candidate(out, "items/book_normal.png".to_string()),
        "glass_bottle" => push_candidate(out, "items/potion_bottle_empty.png".to_string()),
        "fire_charge" => push_candidate(out, "items/fireball.png".to_string()),
        "wooden_sword" => push_candidate(out, "items/wood_sword.png".to_string()),
        "wooden_shovel" => push_candidate(out, "items/wood_shovel.png".to_string()),
        "wooden_pickaxe" => push_candidate(out, "items/wood_pickaxe.png".to_string()),
        "wooden_axe" => push_candidate(out, "items/wood_axe.png".to_string()),
        "wooden_hoe" => push_candidate(out, "items/wood_hoe.png".to_string()),
        "golden_sword" => push_candidate(out, "items/gold_sword.png".to_string()),
        "golden_shovel" => push_candidate(out, "items/gold_shovel.png".to_string()),
        "golden_pickaxe" => push_candidate(out, "items/gold_pickaxe.png".to_string()),
        "golden_axe" => push_candidate(out, "items/gold_axe.png".to_string()),
        "golden_hoe" => push_candidate(out, "items/gold_hoe.png".to_string()),
        "golden_apple" => push_candidate(out, "items/apple_golden.png".to_string()),
        "cooked_porkchop" => push_candidate(out, "items/porkchop_cooked.png".to_string()),
        "cooked_beef" => push_candidate(out, "items/beef_cooked.png".to_string()),
        "cooked_chicken" => push_candidate(out, "items/chicken_cooked.png".to_string()),
        "baked_potato" => push_candidate(out, "items/potato_baked.png".to_string()),
        "poisonous_potato" => push_candidate(out, "items/potato_poisonous.png".to_string()),
        "experience_bottle" => push_candidate(out, "items/experience_bottle.png".to_string()),
        "filled_map" => push_candidate(out, "items/map_filled.png".to_string()),
        "map" => push_candidate(out, "items/map_empty.png".to_string()),
        "firework_charge" => push_candidate(out, "items/fireworks_charge.png".to_string()),
        "lit_furnace" => push_candidate(out, "blocks/furnace_front_on.png".to_string()),
        "furnace" => push_candidate(out, "blocks/furnace_front_off.png".to_string()),
        "grass" => push_candidate(out, "blocks/grass_top.png".to_string()),
        "planks" => push_candidate(out, "blocks/planks_oak.png".to_string()),
        "log" => push_candidate(out, "blocks/log_oak.png".to_string()),
        "log2" => push_candidate(out, "blocks/log_acacia.png".to_string()),
        "leaves" => push_candidate(out, "blocks/leaves_oak.png".to_string()),
        "leaves2" => push_candidate(out, "blocks/leaves_acacia.png".to_string()),
        "flowing_water" | "water" => push_candidate(out, "blocks/water_still.png".to_string()),
        "flowing_lava" | "lava" => push_candidate(out, "blocks/lava_still.png".to_string()),
        "redstone_torch" => push_candidate(out, "blocks/redstone_torch_on.png".to_string()),
        "unlit_redstone_torch" => push_candidate(out, "blocks/redstone_torch_off.png".to_string()),
        "brick_block" => push_candidate(out, "blocks/brick.png".to_string()),
        "mossy_cobblestone" => push_candidate(out, "blocks/cobblestone_mossy.png".to_string()),
        _ => {}
    }

    if let Some(stripped) = key.strip_prefix("wooden_") {
        push_candidate(out, format!("items/wood_{stripped}.png"));
    }
    if let Some(stripped) = key.strip_prefix("golden_") {
        push_candidate(out, format!("items/gold_{stripped}.png"));
    }
}

fn push_candidate(out: &mut Vec<String>, candidate: String) {
    if !out.iter().any(|s| s == &candidate) {
        out.push(candidate);
    }
}

fn wool_color_name(damage: i16) -> &'static str {
    match damage & 0xF {
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
        15 => "black",
        _ => "white",
    }
}

fn dye_color_name(damage: i16) -> &'static str {
    // Dye damage values are reversed relative to wool (0 = ink sac/black, 15 = bone meal/white).
    match damage & 0xF {
        0 => "black",
        1 => "red",
        2 => "green",
        3 => "brown",
        4 => "blue",
        5 => "purple",
        6 => "cyan",
        7 => "silver",
        8 => "gray",
        9 => "pink",
        10 => "lime",
        11 => "yellow",
        12 => "light_blue",
        13 => "magenta",
        14 => "orange",
        _ => "white",
    }
}
