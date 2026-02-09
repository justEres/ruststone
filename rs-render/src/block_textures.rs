#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum TextureKey {
    Stone,
    Dirt,
    GrassTop,
    GrassSide,
    Cobblestone,
    Planks,
    Bedrock,
    Sand,
    Gravel,
    GoldOre,
    IronOre,
    CoalOre,
    LogOak,
    LeavesOak,
    Sponge,
    Glass,
    LapisOre,
    LapisBlock,
    SandstoneTop,
    SandstoneSide,
    SandstoneBottom,
    NoteBlock,
    GoldBlock,
    IronBlock,
    Brick,
    TntTop,
    TntSide,
    TntBottom,
    MossyCobble,
    Obsidian,
    DiamondOre,
    DiamondBlock,
    CraftingTop,
    CraftingSide,
    CraftingFront,
    FurnaceTop,
    FurnaceSide,
    FurnaceFront,
    Ladder,
    CactusTop,
    CactusSide,
    CactusBottom,
    Clay,
    Snow,
    SnowBlock,
    Ice,
    SoulSand,
    Glowstone,
    Netherrack,
    Water,
    Lava,
}

pub const ATLAS_COLUMNS: u32 = 8;
pub const ATLAS_ROWS: u32 = 7;
pub const ATLAS_TEXTURES: [TextureKey; 51] = [
    TextureKey::Stone,
    TextureKey::Dirt,
    TextureKey::GrassTop,
    TextureKey::GrassSide,
    TextureKey::Cobblestone,
    TextureKey::Planks,
    TextureKey::Bedrock,
    TextureKey::Sand,
    TextureKey::Gravel,
    TextureKey::GoldOre,
    TextureKey::IronOre,
    TextureKey::CoalOre,
    TextureKey::LogOak,
    TextureKey::LeavesOak,
    TextureKey::Sponge,
    TextureKey::Glass,
    TextureKey::LapisOre,
    TextureKey::LapisBlock,
    TextureKey::SandstoneTop,
    TextureKey::SandstoneSide,
    TextureKey::SandstoneBottom,
    TextureKey::NoteBlock,
    TextureKey::GoldBlock,
    TextureKey::IronBlock,
    TextureKey::Brick,
    TextureKey::TntTop,
    TextureKey::TntSide,
    TextureKey::TntBottom,
    TextureKey::MossyCobble,
    TextureKey::Obsidian,
    TextureKey::DiamondOre,
    TextureKey::DiamondBlock,
    TextureKey::CraftingTop,
    TextureKey::CraftingSide,
    TextureKey::CraftingFront,
    TextureKey::FurnaceTop,
    TextureKey::FurnaceSide,
    TextureKey::FurnaceFront,
    TextureKey::Ladder,
    TextureKey::CactusTop,
    TextureKey::CactusSide,
    TextureKey::CactusBottom,
    TextureKey::Clay,
    TextureKey::Snow,
    TextureKey::SnowBlock,
    TextureKey::Ice,
    TextureKey::SoulSand,
    TextureKey::Glowstone,
    TextureKey::Netherrack,
    TextureKey::Water,
    TextureKey::Lava,
];

#[derive(Clone, Copy)]
pub enum Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

pub fn texture_for_face(block_id: u16, face: Face) -> TextureKey {
    match block_id {
        1 => TextureKey::Stone,
        2 => match face {
            Face::PosY => TextureKey::GrassTop,
            Face::NegY => TextureKey::Dirt,
            _ => TextureKey::GrassSide,
        },
        3 => TextureKey::Dirt,
        4 => TextureKey::Cobblestone,
        5 => TextureKey::Planks,
        7 => TextureKey::Bedrock,
        8 | 9 => TextureKey::Water,
        10 | 11 => TextureKey::Lava,
        12 => TextureKey::Sand,
        13 => TextureKey::Gravel,
        14 => TextureKey::GoldOre,
        15 => TextureKey::IronOre,
        16 => TextureKey::CoalOre,
        17 => TextureKey::LogOak,
        18 => TextureKey::LeavesOak,
        19 => TextureKey::Sponge,
        20 => TextureKey::Glass,
        21 => TextureKey::LapisOre,
        22 => TextureKey::LapisBlock,
        24 => match face {
            Face::PosY => TextureKey::SandstoneTop,
            Face::NegY => TextureKey::SandstoneBottom,
            _ => TextureKey::SandstoneSide,
        },
        25 => TextureKey::NoteBlock,
        41 => TextureKey::GoldBlock,
        42 => TextureKey::IronBlock,
        45 => TextureKey::Brick,
        46 => match face {
            Face::PosY => TextureKey::TntTop,
            Face::NegY => TextureKey::TntBottom,
            _ => TextureKey::TntSide,
        },
        48 => TextureKey::MossyCobble,
        49 => TextureKey::Obsidian,
        56 => TextureKey::DiamondOre,
        57 => TextureKey::DiamondBlock,
        58 => match face {
            Face::PosY => TextureKey::CraftingTop,
            Face::NegY => TextureKey::Planks,
            Face::PosX => TextureKey::CraftingFront,
            Face::NegX => TextureKey::CraftingFront,
            _ => TextureKey::CraftingSide,
        },
        61 | 62 => match face {
            Face::PosY => TextureKey::FurnaceTop,
            Face::NegY => TextureKey::Stone,
            Face::PosZ => TextureKey::FurnaceFront,
            _ => TextureKey::FurnaceSide,
        },
        65 => TextureKey::Ladder,
        78 => TextureKey::Snow,
        79 => TextureKey::Ice,
        80 => TextureKey::SnowBlock,
        81 => match face {
            Face::PosY => TextureKey::CactusTop,
            Face::NegY => TextureKey::CactusBottom,
            _ => TextureKey::CactusSide,
        },
        82 => TextureKey::Clay,
        87 => TextureKey::Netherrack,
        88 => TextureKey::SoulSand,
        89 => TextureKey::Glowstone,
        _ => TextureKey::Stone,
    }
}

pub fn texture_path(key: TextureKey) -> &'static str {
    match key {
        TextureKey::Stone => "stone.png",
        TextureKey::Dirt => "dirt.png",
        TextureKey::GrassTop => "grass_top.png",
        TextureKey::GrassSide => "grass_side.png",
        TextureKey::Cobblestone => "cobblestone.png",
        TextureKey::Planks => "planks_oak.png",
        TextureKey::Bedrock => "bedrock.png",
        TextureKey::Sand => "sand.png",
        TextureKey::Gravel => "gravel.png",
        TextureKey::GoldOre => "gold_ore.png",
        TextureKey::IronOre => "iron_ore.png",
        TextureKey::CoalOre => "coal_ore.png",
        TextureKey::LogOak => "log_oak.png",
        TextureKey::LeavesOak => "leaves_oak.png",
        TextureKey::Sponge => "sponge.png",
        TextureKey::Glass => "glass.png",
        TextureKey::LapisOre => "lapis_ore.png",
        TextureKey::LapisBlock => "lapis_block.png",
        TextureKey::SandstoneTop => "sandstone_top.png",
        TextureKey::SandstoneSide => "sandstone_normal.png",
        TextureKey::SandstoneBottom => "sandstone_bottom.png",
        TextureKey::NoteBlock => "noteblock.png",
        TextureKey::GoldBlock => "gold_block.png",
        TextureKey::IronBlock => "iron_block.png",
        TextureKey::Brick => "brick.png",
        TextureKey::TntTop => "tnt_top.png",
        TextureKey::TntSide => "tnt_side.png",
        TextureKey::TntBottom => "tnt_bottom.png",
        TextureKey::MossyCobble => "cobblestone_mossy.png",
        TextureKey::Obsidian => "obsidian.png",
        TextureKey::DiamondOre => "diamond_ore.png",
        TextureKey::DiamondBlock => "diamond_block.png",
        TextureKey::CraftingTop => "crafting_table_top.png",
        TextureKey::CraftingSide => "crafting_table_side.png",
        TextureKey::CraftingFront => "crafting_table_front.png",
        TextureKey::FurnaceTop => "furnace_top.png",
        TextureKey::FurnaceSide => "furnace_side.png",
        TextureKey::FurnaceFront => "furnace_front_on.png",
        TextureKey::Ladder => "ladder.png",
        TextureKey::CactusTop => "cactus_top.png",
        TextureKey::CactusSide => "cactus_side.png",
        TextureKey::CactusBottom => "cactus_bottom.png",
        TextureKey::Clay => "clay.png",
        TextureKey::Snow => "snow.png",
        TextureKey::SnowBlock => "snow.png",
        TextureKey::Ice => "ice.png",
        TextureKey::SoulSand => "soul_sand.png",
        TextureKey::Glowstone => "glowstone.png",
        TextureKey::Netherrack => "netherrack.png",
        TextureKey::Water => "water_still.png",
        TextureKey::Lava => "lava_still.png",
    }
}

pub fn uv_for_texture(key: TextureKey) -> [[f32; 2]; 4] {
    atlas_uv_for_texture(key)
}

pub fn is_transparent_texture(key: TextureKey) -> bool {
    matches!(key, TextureKey::Water | TextureKey::Lava)
}

fn atlas_uv_for_texture(key: TextureKey) -> [[f32; 2]; 4] {
    let idx = atlas_index(key) as u32;
    let col = idx % ATLAS_COLUMNS;
    let row = idx / ATLAS_COLUMNS;
    let u0 = col as f32 / ATLAS_COLUMNS as f32;
    let v0 = row as f32 / ATLAS_ROWS as f32;
    let u1 = (col + 1) as f32 / ATLAS_COLUMNS as f32;
    let v1 = (row + 1) as f32 / ATLAS_ROWS as f32;

    let base = base_uv_for_texture(key);
    [
        [u0 + base[0][0] * (u1 - u0), v0 + base[0][1] * (v1 - v0)],
        [u0 + base[1][0] * (u1 - u0), v0 + base[1][1] * (v1 - v0)],
        [u0 + base[2][0] * (u1 - u0), v0 + base[2][1] * (v1 - v0)],
        [u0 + base[3][0] * (u1 - u0), v0 + base[3][1] * (v1 - v0)],
    ]
}

fn base_uv_for_texture(key: TextureKey) -> [[f32; 2]; 4] {
    match key {
        TextureKey::GrassSide => [
            [0.0, 1.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [0.0, 0.0],
        ],
        _ => [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ],
    }
}

fn atlas_index(key: TextureKey) -> usize {
    match key {
        TextureKey::Stone => 0,
        TextureKey::Dirt => 1,
        TextureKey::GrassTop => 2,
        TextureKey::GrassSide => 3,
        TextureKey::Cobblestone => 4,
        TextureKey::Planks => 5,
        TextureKey::Bedrock => 6,
        TextureKey::Sand => 7,
        TextureKey::Gravel => 8,
        TextureKey::GoldOre => 9,
        TextureKey::IronOre => 10,
        TextureKey::CoalOre => 11,
        TextureKey::LogOak => 12,
        TextureKey::LeavesOak => 13,
        TextureKey::Sponge => 14,
        TextureKey::Glass => 15,
        TextureKey::LapisOre => 16,
        TextureKey::LapisBlock => 17,
        TextureKey::SandstoneTop => 18,
        TextureKey::SandstoneSide => 19,
        TextureKey::SandstoneBottom => 20,
        TextureKey::NoteBlock => 21,
        TextureKey::GoldBlock => 22,
        TextureKey::IronBlock => 23,
        TextureKey::Brick => 24,
        TextureKey::TntTop => 25,
        TextureKey::TntSide => 26,
        TextureKey::TntBottom => 27,
        TextureKey::MossyCobble => 28,
        TextureKey::Obsidian => 29,
        TextureKey::DiamondOre => 30,
        TextureKey::DiamondBlock => 31,
        TextureKey::CraftingTop => 32,
        TextureKey::CraftingSide => 33,
        TextureKey::CraftingFront => 34,
        TextureKey::FurnaceTop => 35,
        TextureKey::FurnaceSide => 36,
        TextureKey::FurnaceFront => 37,
        TextureKey::Ladder => 38,
        TextureKey::CactusTop => 39,
        TextureKey::CactusSide => 40,
        TextureKey::CactusBottom => 41,
        TextureKey::Clay => 42,
        TextureKey::Snow => 43,
        TextureKey::SnowBlock => 44,
        TextureKey::Ice => 45,
        TextureKey::SoulSand => 46,
        TextureKey::Glowstone => 47,
        TextureKey::Netherrack => 48,
        TextureKey::Water => 49,
        TextureKey::Lava => 50,
    }
}

#[derive(Clone, Copy)]
pub struct BiomeTint {
    pub grass: [f32; 4],
    pub foliage: [f32; 4],
    pub water: [f32; 4],
}

pub fn biome_tint(biome_id: u8) -> BiomeTint {
    match biome_id {
        2 | 17 => BiomeTint {
            grass: rgb(0.91, 0.77, 0.38),
            foliage: rgb(0.85, 0.74, 0.4),
            water: rgb(0.25, 0.42, 0.8),
        },
        6 => BiomeTint {
            grass: rgb(0.4, 0.56, 0.2),
            foliage: rgb(0.35, 0.5, 0.2),
            water: rgb(0.2, 0.36, 0.5),
        },
        5 | 19 | 20 | 30 | 31 => BiomeTint {
            grass: rgb(0.5, 0.6, 0.5),
            foliage: rgb(0.45, 0.55, 0.45),
            water: rgb(0.25, 0.42, 0.8),
        },
        8 | 9 => BiomeTint {
            grass: rgb(0.3, 0.3, 0.3),
            foliage: rgb(0.25, 0.25, 0.25),
            water: rgb(0.4, 0.1, 0.1),
        },
        12 | 140 => BiomeTint {
            grass: rgb(0.8, 0.8, 0.9),
            foliage: rgb(0.8, 0.8, 0.9),
            water: rgb(0.25, 0.42, 0.8),
        },
        21 | 22 | 23 => BiomeTint {
            grass: rgb(0.2, 0.6, 0.2),
            foliage: rgb(0.2, 0.55, 0.2),
            water: rgb(0.25, 0.42, 0.8),
        },
        35 | 36 => BiomeTint {
            grass: rgb(0.5, 0.7, 0.2),
            foliage: rgb(0.45, 0.65, 0.2),
            water: rgb(0.25, 0.42, 0.8),
        },
        37 | 38 | 39 => BiomeTint {
            grass: rgb(0.75, 0.65, 0.4),
            foliage: rgb(0.6, 0.55, 0.35),
            water: rgb(0.25, 0.42, 0.8),
        },
        _ => BiomeTint {
            grass: rgb(0.36, 0.74, 0.29),
            foliage: rgb(0.28, 0.7, 0.22),
            water: rgb(0.25, 0.42, 0.8),
        },
    }
}

fn rgb(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}
