use base64::Engine;
use rs_protocol::format::Component;
use rs_protocol::format::ComponentType;
use rs_protocol::format::color::Color;
use rs_protocol::protocol::{Conn, packet::Packet};
use rs_protocol::types::Value as MetadataValue;
use rs_utils::{
    BlockUpdate, FromNetMessage, InventoryEnchantment, InventoryItemMeta, InventoryItemStack,
    InventoryMessage, InventoryWindowInfo, MobKind, NetEntityAnimation, NetEntityKind,
    NetEntityMessage, ObjectKind, PlayerPosition, PlayerSkinModel, ScoreboardMessage,
    SoundCategory, SoundEvent, TitleMessage, item_name,
};
use tracing::{debug, info, warn};

use crate::chunk_decode;

mod audio;
mod chat;
mod entities;
mod inventory;
mod join_game;
mod scoreboard;
mod title;
mod world;

fn send_join_game(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
    entity_id: i32,
    gamemode: u8,
    requested_view_distance: u8,
) {
    if let Err(err) = rs_protocol::protocol::packet::send_client_settings(
        conn,
        "en_US".to_string(),
        requested_view_distance.clamp(2, 64),
        0,
        true,
        0x7f,
        rs_protocol::protocol::packet::Hand::MainHand,
    ) {
        warn!(
            "Failed to send initial ClientSettings after JoinGame: {}",
            err
        );
    }
    let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::LocalPlayerId {
        entity_id,
    }));
    let _ = to_main.send(FromNetMessage::GameMode { gamemode });
}

fn log_join_game(
    entity_id: i32,
    gamemode: u8,
    dimension: Option<i32>,
    difficulty: Option<u8>,
    max_players: u8,
    level_type: Option<&str>,
    server_view_distance: Option<i32>,
    reduced_debug_info: Option<bool>,
    enable_respawn_screen: Option<bool>,
    requested_view_distance: u8,
) {
    info!(
        entity_id,
        gamemode,
        dimension,
        difficulty,
        max_players,
        level_type,
        server_view_distance,
        reduced_debug_info,
        enable_respawn_screen,
        requested_view_distance,
        "JoinGame"
    );
}

fn send_player_position(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    position: Option<(f64, f64, f64)>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    flags: Option<u8>,
    on_ground: Option<bool>,
) {
    let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
        position,
        yaw,
        pitch,
        flags,
        on_ground,
    }));
}

fn send_spawn_player(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    entity_id: i32,
    uuid: Option<rs_protocol::protocol::UUID>,
    pos: bevy::prelude::Vec3,
    yaw_i8: i8,
    pitch_i8: i8,
) {
    let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
        entity_id,
        uuid,
        kind: NetEntityKind::Player,
        pos,
        yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(yaw_i8)),
        pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(pitch_i8)),
        on_ground: None,
    }));
}

pub fn handle_packet(
    pkt: Packet,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
    requested_view_distance: u8,
) {
    match pkt {
        Packet::TeleportPlayer_WithConfirm(tp) => {
            send_player_position(
                to_main,
                Some((tp.x, tp.y, tp.z)),
                Some(tp.yaw),
                Some(tp.pitch),
                Some(tp.flags),
                None,
            );
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::TeleportConfirm {
                    teleport_id: tp.teleport_id,
                },
            );
        }
        Packet::JoinGame_i8(_)
        | Packet::JoinGame_i8_NoDebug(_)
        | Packet::JoinGame_i32(_)
        | Packet::JoinGame_i32_ViewDistance(_)
        | Packet::KeepAliveClientbound_VarInt(_)
        | Packet::UpdateHealth(_)
        | Packet::UpdateHealth_u16(_)
        | Packet::SetExperience(_)
        | Packet::SetExperience_i16(_)
        | Packet::ChangeGameState(_)
        | Packet::TimeUpdate(_)
        | Packet::Respawn_Gamemode(_)
        | Packet::Respawn_HashedSeed(_)
        | Packet::Respawn_NBT(_)
        | Packet::Respawn_WorldName(_)
        | Packet::UpdateViewDistance(_)
        | Packet::PlayerAbilities(_) => {
            join_game::handle_packet(pkt, to_main, conn, requested_view_distance)
        }
        Packet::ChunkData(_)
        | Packet::ChunkData_NoEntities(_)
        | Packet::ChunkData_NoEntities_u16(_)
        | Packet::ChunkDataBulk(_)
        | Packet::ChunkUnload(_)
        | Packet::BlockChange_VarInt(_)
        | Packet::BlockChange_u8(_)
        | Packet::MultiBlockChange_VarInt(_)
        | Packet::MultiBlockChange_u16(_)
        | Packet::UpdateBlockEntity(_) => world::handle_packet(pkt, to_main),
        Packet::TeleportPlayer_NoConfirm(_)
        | Packet::TeleportPlayer_OnGround(_)
        | Packet::PlayerPosition(_)
        | Packet::PlayerPosition_HeadY(_)
        | Packet::PlayerPositionLook(_)
        | Packet::PlayerPositionLook_HeadY(_)
        | Packet::PlayerLook(_)
        | Packet::SpawnPlayer_i32_HeldItem(_)
        | Packet::SpawnPlayer_i32(_)
        | Packet::SpawnPlayer_f64(_)
        | Packet::SpawnPlayer_f64_NoMeta(_)
        | Packet::SpawnPlayer_i32_HeldItem_String(_)
        | Packet::EntityMetadata(_)
        | Packet::EntityMetadata_i32(_)
        | Packet::Animation(_)
        | Packet::EntityProperties(_)
        | Packet::SpawnObject_i32_NoUUID(_)
        | Packet::SpawnObject_i32(_)
        | Packet::SpawnExperienceOrb_i32(_)
        | Packet::SpawnMob_u8_i32_NoUUID(_)
        | Packet::SpawnMob_u8_i32(_)
        | Packet::SpawnMob_u8(_)
        | Packet::EntityHeadLook(_)
        | Packet::EntityHeadLook_i32(_)
        | Packet::EntityMove_i8(_)
        | Packet::EntityMove_i8_i32_NoGround(_)
        | Packet::EntityVelocity(_)
        | Packet::EntityVelocity_i32(_)
        | Packet::EntityTeleport_i32(_)
        | Packet::EntityTeleport_i32_i32_NoGround(_)
        | Packet::EntityEquipment_u16(_)
        | Packet::EntityEquipment_u16_i32(_)
        | Packet::EntityLookAndMove_i8(_)
        | Packet::EntityLookAndMove_i8_i32_NoGround(_)
        | Packet::EntityLook_VarInt(_)
        | Packet::EntityLook_i32_NoGround(_)
        | Packet::EntityDestroy(_)
        | Packet::EntityDestroy_u8(_)
        | Packet::EntityStatus(_)
        | Packet::CollectItem_nocount(_)
        | Packet::CollectItem_nocount_i32(_)
        | Packet::PlayerInfo(_)
        | Packet::EntityEffect(_)
        | Packet::EntityEffect_i32(_)
        | Packet::EntityRemoveEffect(_)
        | Packet::EntityRemoveEffect_i32(_) => entities::handle_packet(pkt, to_main),
        Packet::NamedSoundEffect(_)
        | Packet::NamedSoundEffect_u8(_)
        | Packet::NamedSoundEffect_u8_NoCategory(_)
        | Packet::Effect(_)
        | Packet::Effect_u8y(_)
        | Packet::Explosion(_) => audio::handle_packet(pkt, to_main),
        Packet::ServerMessage_NoPosition(_)
        | Packet::ServerMessage_Position(_)
        | Packet::ServerMessage_Sender(_)
        | Packet::Disconnect(_)
        | Packet::TabCompleteReply(_)
        | Packet::PlayerListHeaderFooter(_) => chat::handle_packet(pkt, to_main),
        Packet::WindowOpen(_)
        | Packet::WindowOpen_u8(_)
        | Packet::WindowOpen_VarInt(_)
        | Packet::WindowOpenHorse(_)
        | Packet::WindowClose(_)
        | Packet::WindowItems(_)
        | Packet::WindowSetSlot(_)
        | Packet::ConfirmTransaction(_)
        | Packet::SetCurrentHotbarSlot(_) => inventory::handle_packet(pkt, to_main),
        Packet::Title(_) | Packet::Title_notext(_) | Packet::Title_notext_component(_) => {
            title::handle_packet(pkt, to_main)
        }
        Packet::ScoreboardDisplay(_)
        | Packet::ScoreboardObjective(_)
        | Packet::ScoreboardObjective_NoMode(_)
        | Packet::UpdateScore(_)
        | Packet::UpdateScore_i32(_)
        | Packet::Teams_u8_NameTagVisibility(_)
        | Packet::Teams_u8(_)
        | Packet::Teams_NoVisColor(_)
        | Packet::Teams_VarInt(_) => scoreboard::handle_packet(pkt, to_main),
        _other => {}
    }
}

fn send_title_packet(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    action: i32,
    title: Option<&Component>,
    subtitle: Option<&Component>,
    action_bar_text: Option<&str>,
    fade_in: Option<i32>,
    fade_stay: Option<i32>,
    fade_out: Option<i32>,
) {
    match action {
        0 => {
            if let Some(title) = title {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetTitle {
                    text: component_to_legacy(title),
                }));
            }
        }
        1 => {
            if let Some(subtitle) = subtitle {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetSubtitle {
                    text: component_to_legacy(subtitle),
                }));
            }
        }
        2 => {
            let text = action_bar_text
                .map(ToString::to_string)
                .or_else(|| title.map(component_to_legacy))
                .or_else(|| subtitle.map(component_to_legacy));
            if let Some(text) = text {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetActionBar { text }));
            }
        }
        3 => {
            send_title_times(to_main, fade_in, fade_stay, fade_out);
        }
        4 => {
            let _ = to_main.send(FromNetMessage::Title(TitleMessage::Clear));
        }
        5 => {
            let _ = to_main.send(FromNetMessage::Title(TitleMessage::Reset));
        }
        _ => {}
    }
}

fn send_title_times(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    fade_in: Option<i32>,
    fade_stay: Option<i32>,
    fade_out: Option<i32>,
) {
    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetTimes {
        fade_in_ticks: fade_in.unwrap_or(10),
        stay_ticks: fade_stay.unwrap_or(70),
        fade_out_ticks: fade_out.unwrap_or(20),
    }));
}

fn send_team_packet(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    name: String,
    mode: u8,
    display_name: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    players: Option<Vec<String>>,
) {
    let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Team {
        name,
        mode,
        display_name,
        prefix,
        suffix,
        players,
    }));
}

fn angle_i8_to_degrees(angle: i8) -> f32 {
    angle as f32 * (360.0 / 256.0)
}

fn server_yaw_to_client_yaw(yaw_deg: f32) -> f32 {
    std::f32::consts::PI - yaw_deg.to_radians()
}

fn server_pitch_to_client_pitch(pitch_deg: f32) -> f32 {
    -pitch_deg.to_radians()
}

fn block_state_to_id_meta(block_state: i32) -> u16 {
    if block_state <= 0 {
        0
    } else {
        (block_state as u32 & 0xFFFF) as u16
    }
}

fn protocol_stack_to_inventory_item(
    stack: Option<rs_protocol::item::Stack>,
) -> Option<InventoryItemStack> {
    stack.map(|s| {
        let meta = protocol_stack_meta_to_inventory_meta(&s);
        InventoryItemStack {
            item_id: s.id as i32,
            count: s.count.clamp(0, u8::MAX as isize) as u8,
            damage: s
                .damage
                .unwrap_or(0)
                .clamp(i16::MIN as isize, i16::MAX as isize) as i16,
            meta,
        }
    })
}

fn protocol_stack_meta_to_inventory_meta(stack: &rs_protocol::item::Stack) -> InventoryItemMeta {
    let display_name = stack
        .meta
        .display_name()
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty());
    let lore = stack
        .meta
        .lore()
        .into_iter()
        .map(|line| line.to_string())
        .filter(|line| !line.is_empty())
        .collect();
    let enchantments = stack
        .meta
        .raw_enchantments()
        .into_iter()
        .map(|(id, level)| InventoryEnchantment { id, level })
        .collect();
    InventoryItemMeta {
        display_name,
        lore,
        display_color: stack.meta.display_color().map(|color| color as u32),
        enchantments,
        repair_cost: stack.meta.repair_cost(),
        unbreakable: stack.meta.unbreakable(),
    }
}

fn handle_entity_metadata(
    entity_id: i32,
    metadata: &rs_protocol::types::Metadata,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
) {
    if let Some(MetadataValue::Byte(flags)) = metadata.get_raw(0) {
        let sneaking = (*flags & 0x02) != 0;
        let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Pose {
            entity_id,
            sneaking,
        }));
    }

    if let Some(MetadataValue::OptionalItemStack(stack_opt)) = metadata.get_raw(10) {
        let stack_converted = protocol_stack_to_inventory_item(stack_opt.clone());
        debug!(
            entity_id,
            has_stack = stack_converted.is_some(),
            item_id = stack_converted.as_ref().map(|s| s.item_id),
            damage = stack_converted.as_ref().map(|s| s.damage),
            count = stack_converted.as_ref().map(|s| s.count),
            "entity metadata updated item stack slot"
        );

        let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::SetItemStack {
            entity_id,
            stack: stack_converted,
        }));

        if let Some(stack) = stack_opt.as_ref() {
            let label = item_stack_label(stack);
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::SetLabel {
                entity_id,
                label,
            }));
        }
    }

    if let Some(MetadataValue::Byte(sheep_flags)) = metadata.get_raw(16) {
        let fleece_color = (*sheep_flags & 0x0F) as u8;
        let sheared = (*sheep_flags & 0x10) != 0;
        let _ = to_main.send(FromNetMessage::NetEntity(
            NetEntityMessage::SheepAppearance {
                entity_id,
                fleece_color,
                sheared,
            },
        ));
    }
}

fn component_to_legacy(component: &Component) -> String {
    let mut out = String::new();
    for part in &component.list {
        let modifier = part.get_modifier();
        if let Some(code) = legacy_color_code(modifier.color) {
            out.push('§');
            out.push(code);
        }
        if modifier.bold {
            out.push_str("§l");
        }
        if modifier.italic {
            out.push_str("§o");
        }
        if modifier.underlined {
            out.push_str("§n");
        }
        if modifier.strikethrough {
            out.push_str("§m");
        }
        if modifier.obfuscated {
            out.push_str("§k");
        }
        out.push_str(match part {
            ComponentType::Text { text, .. } => text,
            ComponentType::Hover { text, .. } => text,
            ComponentType::Click { text, .. } => text,
            ComponentType::ClickAndHover { text, .. } => text,
        });
    }
    if out.is_empty() {
        component.to_string()
    } else {
        out
    }
}

fn legacy_color_code(color: Color) -> Option<char> {
    match color {
        Color::Black => Some('0'),
        Color::DarkBlue => Some('1'),
        Color::DarkGreen => Some('2'),
        Color::DarkAqua => Some('3'),
        Color::DarkRed => Some('4'),
        Color::DarkPurple => Some('5'),
        Color::Gold => Some('6'),
        Color::Gray => Some('7'),
        Color::DarkGray => Some('8'),
        Color::Blue => Some('9'),
        Color::Green => Some('a'),
        Color::Aqua => Some('b'),
        Color::Red => Some('c'),
        Color::LightPurple => Some('d'),
        Color::Yellow => Some('e'),
        Color::White => Some('f'),
        Color::Reset => Some('r'),
        Color::RGB(_) | Color::None => None,
    }
}

fn item_stack_label(stack: &rs_protocol::item::Stack) -> String {
    let name = stack
        .meta
        .display_name()
        .map(|c| c.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| item_name(stack.id as i32).to_string());
    if stack.count > 1 {
        format!("{name} x{}", stack.count)
    } else {
        name
    }
}

fn extract_skin_info_from_player_properties(
    properties: &[rs_protocol::protocol::packet::PlayerProperty],
) -> (Option<String>, PlayerSkinModel) {
    extract_skin_info_from_properties(
        properties
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str())),
    )
}

fn extract_skin_info_from_spawn_properties(
    properties: &[rs_protocol::protocol::packet::SpawnProperty],
) -> (Option<String>, PlayerSkinModel) {
    extract_skin_info_from_properties(
        properties
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str())),
    )
}

fn extract_skin_info_from_properties<'a>(
    properties: impl Iterator<Item = (&'a str, &'a str)>,
) -> (Option<String>, PlayerSkinModel) {
    for (name, value) in properties {
        if name != "textures" {
            continue;
        }
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value) else {
            continue;
        };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&decoded) else {
            continue;
        };
        let Some(url) = json.pointer("/textures/SKIN/url").and_then(|v| v.as_str()) else {
            continue;
        };
        let skin_model = match json
            .pointer("/textures/SKIN/metadata/model")
            .and_then(|v| v.as_str())
        {
            Some("slim") => PlayerSkinModel::Slim,
            _ => PlayerSkinModel::Classic,
        };
        if url.starts_with("http://textures.minecraft.net/texture/")
            || url.starts_with("https://textures.minecraft.net/texture/")
        {
            let normalized = url.replacen("http://", "https://", 1);
            return (Some(normalized), skin_model);
        }
    }
    (None, PlayerSkinModel::Classic)
}

fn packet_sound_position(x: i32, y: i32, z: i32) -> bevy::prelude::Vec3 {
    bevy::prelude::Vec3::new(x as f32 / 8.0, y as f32 / 8.0, z as f32 / 8.0)
}

fn send_aux_sound_effect(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    effect_id: i32,
    position: bevy::prelude::Vec3,
    data: i32,
) {
    let mapped = match effect_id {
        1000 => Some(("minecraft:random.click", SoundCategory::Block, 1.0, 1.0)),
        1001 => Some(("minecraft:random.click", SoundCategory::Block, 1.0, 1.2)),
        1002 => Some(("minecraft:random.bow", SoundCategory::Player, 1.0, 1.2)),
        1003 => Some(("minecraft:random.door_open", SoundCategory::Block, 1.0, 1.0)),
        1004 => Some(("minecraft:random.fizz", SoundCategory::Block, 0.5, 2.6)),
        1005 => record_name_from_item_id(data).map(|name| (name, SoundCategory::Record, 4.0, 1.0)),
        1006 => Some((
            "minecraft:random.door_close",
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1007 => Some((
            "minecraft:mob.ghast.charge",
            SoundCategory::Hostile,
            10.0,
            1.0,
        )),
        1008 => Some((
            "minecraft:mob.ghast.fireball",
            SoundCategory::Hostile,
            10.0,
            1.0,
        )),
        1009 => Some((
            "minecraft:mob.ghast.fireball",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1010 => Some((
            "minecraft:mob.zombie.wood",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1011 => Some((
            "minecraft:mob.zombie.metal",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1012 => Some((
            "minecraft:mob.zombie.woodbreak",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1013 => Some((
            "minecraft:mob.wither.spawn",
            SoundCategory::Hostile,
            1.0,
            1.0,
        )),
        1014 => Some((
            "minecraft:mob.wither.shoot",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1015 => Some((
            "minecraft:mob.bat.takeoff",
            SoundCategory::Ambient,
            0.05,
            1.0,
        )),
        1016 => Some((
            "minecraft:mob.zombie.infect",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1017 => Some((
            "minecraft:mob.zombie.unfect",
            SoundCategory::Hostile,
            2.0,
            1.0,
        )),
        1018 => Some((
            "minecraft:mob.enderdragon.end",
            SoundCategory::Hostile,
            5.0,
            1.0,
        )),
        1020 => Some((
            "minecraft:random.anvil_break",
            SoundCategory::Block,
            1.0,
            1.0,
        )),
        1021 => Some(("minecraft:random.anvil_use", SoundCategory::Block, 1.0, 1.0)),
        1022 => Some((
            "minecraft:random.anvil_land",
            SoundCategory::Block,
            0.3,
            1.0,
        )),
        _ => None,
    };
    if let Some((event_id, category, volume, pitch)) = mapped {
        let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
            event_id: event_id.to_string(),
            position,
            volume,
            pitch,
            category_override: Some(category),
            distance_delay: false,
        }));
    }
}

fn record_name_from_item_id(item_id: i32) -> Option<&'static str> {
    match item_id {
        2256 => Some("minecraft:records.13"),
        2257 => Some("minecraft:records.cat"),
        2258 => Some("minecraft:records.blocks"),
        2259 => Some("minecraft:records.chirp"),
        2260 => Some("minecraft:records.far"),
        2261 => Some("minecraft:records.mall"),
        2262 => Some("minecraft:records.mellohi"),
        2263 => Some("minecraft:records.stal"),
        2264 => Some("minecraft:records.strad"),
        2265 => Some("minecraft:records.ward"),
        2266 => Some("minecraft:records.11"),
        2267 => Some("minecraft:records.wait"),
        _ => None,
    }
}

fn mob_type_to_kind(ty: u8) -> MobKind {
    match ty {
        50 => MobKind::Creeper,
        51 => MobKind::Skeleton,
        52 => MobKind::Spider,
        53 => MobKind::Giant,
        54 => MobKind::Zombie,
        55 => MobKind::Slime,
        56 => MobKind::Ghast,
        57 => MobKind::PigZombie,
        58 => MobKind::Enderman,
        59 => MobKind::CaveSpider,
        60 => MobKind::Silverfish,
        61 => MobKind::Blaze,
        62 => MobKind::MagmaCube,
        63 => MobKind::EnderDragon,
        64 => MobKind::Wither,
        65 => MobKind::Bat,
        66 => MobKind::Witch,
        67 => MobKind::Endermite,
        68 => MobKind::Guardian,
        90 => MobKind::Pig,
        91 => MobKind::Sheep,
        92 => MobKind::Cow,
        93 => MobKind::Chicken,
        94 => MobKind::Squid,
        95 => MobKind::Wolf,
        96 => MobKind::Mooshroom,
        97 => MobKind::SnowGolem,
        98 => MobKind::Ocelot,
        99 => MobKind::IronGolem,
        100 => MobKind::Horse,
        101 => MobKind::Rabbit,
        120 => MobKind::Villager,
        other => MobKind::Unknown(other),
    }
}

fn object_type_to_kind(ty: u8) -> NetEntityKind {
    match ty {
        2 => NetEntityKind::Item,
        10 => NetEntityKind::Object(ObjectKind::Minecart),
        1 => NetEntityKind::Object(ObjectKind::Boat),
        60 => NetEntityKind::Object(ObjectKind::Arrow),
        61 => NetEntityKind::Object(ObjectKind::Snowball),
        71 => NetEntityKind::Object(ObjectKind::ItemFrame),
        77 => NetEntityKind::Object(ObjectKind::LeashKnot),
        65 => NetEntityKind::Object(ObjectKind::EnderPearl),
        72 => NetEntityKind::Object(ObjectKind::EnderEye),
        76 => NetEntityKind::Object(ObjectKind::Firework),
        63 => NetEntityKind::Object(ObjectKind::LargeFireball),
        64 => NetEntityKind::Object(ObjectKind::SmallFireball),
        66 => NetEntityKind::Object(ObjectKind::WitherSkull),
        62 => NetEntityKind::Object(ObjectKind::Egg),
        73 => NetEntityKind::Object(ObjectKind::SplashPotion),
        75 => NetEntityKind::Object(ObjectKind::ExpBottle),
        90 => NetEntityKind::Object(ObjectKind::FishingHook),
        50 => NetEntityKind::Object(ObjectKind::PrimedTnt),
        78 => NetEntityKind::Object(ObjectKind::ArmorStand),
        51 => NetEntityKind::Object(ObjectKind::EndCrystal),
        70 => NetEntityKind::Object(ObjectKind::FallingBlock),
        other => NetEntityKind::Object(ObjectKind::Unknown(other)),
    }
}
