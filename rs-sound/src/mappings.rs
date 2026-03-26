use rs_utils::SoundCategory;

use crate::DEFAULT_SOUND_EVENT;

pub fn auxiliary_effect_to_sound(
    effect_id: i32,
    data: i32,
) -> Option<(String, SoundCategory, f32, f32)> {
    match effect_id {
        1000 => Some((
            "minecraft:random.click".to_string(),
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1001 => Some((
            "minecraft:random.click".to_string(),
            SoundCategory::Block,
            1.0,
            1.2,
        )),
        1002 => Some((
            "minecraft:random.bow".to_string(),
            SoundCategory::Player,
            1.0,
            1.2,
        )),
        1003 => Some((
            "minecraft:random.door_open".to_string(),
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1004 => Some((
            "minecraft:random.fizz".to_string(),
            SoundCategory::Block,
            0.5,
            2.6,
        )),
        1005 => Some((
            format!("minecraft:records.{}", record_name_from_item_id(data)?),
            SoundCategory::Record,
            4.0,
            1.0,
        )),
        1006 => Some((
            "minecraft:random.door_close".to_string(),
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1007 => Some((
            "minecraft:mob.ghast.charge".to_string(),
            SoundCategory::Hostile,
            10.0,
            1.0,
        )),
        1008 | 1009 => Some((
            "minecraft:mob.ghast.fireball".to_string(),
            SoundCategory::Hostile,
            if effect_id == 1008 { 10.0 } else { 2.0 },
            1.0,
        )),
        1010 => Some((
            "minecraft:mob.zombie.wood".to_string(),
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1011 => Some((
            "minecraft:mob.zombie.metal".to_string(),
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1012 => Some((
            "minecraft:mob.zombie.woodbreak".to_string(),
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1013 => Some((
            "minecraft:mob.wither.spawn".to_string(),
            SoundCategory::Hostile,
            1.0,
            1.0,
        )),
        1014 => Some((
            "minecraft:mob.wither.shoot".to_string(),
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1015 => Some((
            "minecraft:mob.bat.takeoff".to_string(),
            SoundCategory::Ambient,
            0.05,
            1.0,
        )),
        1016 => Some((
            "minecraft:mob.zombie.infect".to_string(),
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1017 => Some((
            "minecraft:mob.zombie.unfect".to_string(),
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1018 => Some((
            "minecraft:mob.enderdragon.end".to_string(),
            SoundCategory::Hostile,
            5.0,
            1.0,
        )),
        1020 => Some((
            "minecraft:random.anvil_break".to_string(),
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1021 => Some((
            "minecraft:random.anvil_use".to_string(),
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1022 => Some((
            "minecraft:random.anvil_land".to_string(),
            SoundCategory::Block,
            0.3,
            1.0,
        )),
        _ => None,
    }
}

fn record_name_from_item_id(item_id: i32) -> Option<&'static str> {
    match item_id {
        2256 => Some("13"),
        2257 => Some("cat"),
        2258 => Some("blocks"),
        2259 => Some("chirp"),
        2260 => Some("far"),
        2261 => Some("mall"),
        2262 => Some("mellohi"),
        2263 => Some("stal"),
        2264 => Some("strad"),
        2265 => Some("ward"),
        2266 => Some("11"),
        2267 => Some("wait"),
        _ => None,
    }
}

pub fn block_step_sound(block_id: u16) -> &'static str {
    match block_id {
        2 | 3 | 31 | 37 | 38 | 39 | 40 | 175 => "minecraft:step.grass",
        12 | 13 | 82 => "minecraft:step.sand",
        18 | 30 | 106 => "minecraft:step.cloth",
        78 | 80 => "minecraft:step.snow",
        5
        | 17
        | 47
        | 53
        | 54
        | 58
        | 63
        | 64
        | 65
        | 68
        | 72
        | 84
        | 85
        | 86
        | 91
        | 96
        | 107
        | 125
        | 126
        | 130
        | 134..=136
        | 143
        | 146
        | 158
        | 162
        | 163
        | 164
        | 183..=188 => "minecraft:step.wood",
        8 | 9 => "minecraft:liquid.splash",
        10 | 11 => "minecraft:liquid.lava",
        _ => "minecraft:step.stone",
    }
}

pub fn block_dig_sound(block_id: u16) -> &'static str {
    match block_id {
        2 | 3 | 31 | 37 | 38 | 39 | 40 | 175 => "minecraft:dig.grass",
        12 | 13 | 82 => "minecraft:dig.sand",
        18 | 30 | 35 | 171 => "minecraft:dig.cloth",
        78 | 80 => "minecraft:dig.snow",
        5
        | 17
        | 47
        | 53
        | 54
        | 58
        | 63
        | 64
        | 65
        | 68
        | 72
        | 84
        | 85
        | 86
        | 91
        | 96
        | 107
        | 125
        | 126
        | 130
        | 134..=136
        | 143
        | 146
        | 158
        | 162
        | 163
        | 164
        | 183..=188 => "minecraft:dig.wood",
        _ => "minecraft:dig.stone",
    }
}

pub fn button_press_sound_id() -> &'static str {
    DEFAULT_SOUND_EVENT
}
