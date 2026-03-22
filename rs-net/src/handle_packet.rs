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
use tracing::{debug, warn};

use crate::chunk_decode;

fn send_join_game(
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
    entity_id: i32,
    gamemode: u8,
) {
    if let Err(err) = rs_protocol::protocol::packet::send_client_settings(
        conn,
        "en_US".to_string(),
        12,
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
) {
    use rs_protocol::protocol::packet::Packet;
    match pkt {
        Packet::JoinGame_i8(jg) => send_join_game(to_main, conn, jg.entity_id, jg.gamemode),
        Packet::JoinGame_i8_NoDebug(jg) => send_join_game(to_main, conn, jg.entity_id, jg.gamemode),
        Packet::JoinGame_i32(jg) => send_join_game(to_main, conn, jg.entity_id, jg.gamemode),
        Packet::JoinGame_i32_ViewDistance(jg) => {
            send_join_game(to_main, conn, jg.entity_id, jg.gamemode)
        }
        Packet::ChunkData(cd) => {
            let bitmask = cd.bitmask.0 as u16;
            match chunk_decode::decode_chunk(
                cd.chunk_x,
                cd.chunk_z,
                cd.new,
                bitmask,
                &cd.data.data,
                true,
            ) {
                Ok((chunk, _)) => {
                    let _ = to_main.send(FromNetMessage::ChunkData(chunk));
                }
                Err(err) => {
                    warn!("Failed to decode ChunkData: {}", err);
                }
            }
        }
        Packet::ChunkDataBulk(cdb) => {
            let mut offset = 0usize;
            for meta in cdb.chunk_meta.data.iter() {
                match chunk_decode::decode_chunk(
                    meta.x,
                    meta.z,
                    true,
                    meta.bitmask,
                    &cdb.chunk_data[offset..],
                    cdb.skylight,
                ) {
                    Ok((chunk, consumed)) => {
                        offset += consumed;
                        let _ = to_main.send(FromNetMessage::ChunkData(chunk));
                    }
                    Err(err) => {
                        warn!("Failed to decode ChunkDataBulk: {}", err);
                        break;
                    }
                }
            }
        }
        Packet::ChunkUnload(unload) => {
            let _ = to_main.send(FromNetMessage::ChunkUnload {
                x: unload.x,
                z: unload.z,
            });
        }
        Packet::TeleportPlayer_NoConfirm(tp) => send_player_position(
            to_main,
            Some((tp.x, tp.y, tp.z)),
            Some(tp.yaw),
            Some(tp.pitch),
            Some(tp.flags),
            None,
        ),
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
        Packet::TeleportPlayer_OnGround(tp) => send_player_position(
            to_main,
            Some((tp.x, tp.eyes_y, tp.z)),
            Some(tp.yaw),
            Some(tp.pitch),
            None,
            Some(tp.on_ground),
        ),
        Packet::PlayerPosition(position) => send_player_position(
            to_main,
            Some((position.x, position.y, position.z)),
            None,
            None,
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerPosition_HeadY(position) => send_player_position(
            to_main,
            Some((position.x, position.feet_y, position.z)),
            None,
            None,
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerPositionLook(position) => send_player_position(
            to_main,
            Some((position.x, position.y, position.z)),
            Some(position.yaw),
            Some(position.pitch),
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerPositionLook_HeadY(position) => send_player_position(
            to_main,
            Some((position.x, position.feet_y, position.z)),
            Some(position.yaw),
            Some(position.pitch),
            None,
            Some(position.on_ground),
        ),
        Packet::PlayerLook(position) => send_player_position(
            to_main,
            None,
            Some(position.yaw),
            Some(position.pitch),
            None,
            Some(position.on_ground),
        ),
        Packet::SpawnPlayer_i32_HeldItem(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(
                f64::from(sp.x) as f32,
                f64::from(sp.y) as f32,
                f64::from(sp.z) as f32,
            ),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_i32(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(
                f64::from(sp.x) as f32,
                f64::from(sp.y) as f32,
                f64::from(sp.z) as f32,
            ),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_f64(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(sp.x as f32, sp.y as f32, sp.z as f32),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_f64_NoMeta(sp) => send_spawn_player(
            to_main,
            sp.entity_id.0,
            Some(sp.uuid),
            bevy::prelude::Vec3::new(sp.x as f32, sp.y as f32, sp.z as f32),
            sp.yaw,
            sp.pitch,
        ),
        Packet::SpawnPlayer_i32_HeldItem_String(sp) => {
            let parsed_uuid = sp.uuid.parse::<rs_protocol::protocol::UUID>().ok();
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sp.entity_id.0,
                uuid: parsed_uuid.clone(),
                kind: NetEntityKind::Player,
                pos: bevy::prelude::Vec3::new(
                    f64::from(sp.x) as f32,
                    f64::from(sp.y) as f32,
                    f64::from(sp.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sp.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sp.pitch)),
                on_ground: None,
            }));
            if let Some(uuid) = parsed_uuid {
                let (skin_url, skin_model) =
                    extract_skin_info_from_spawn_properties(&sp.properties.data);
                debug!(
                    "NET SpawnPlayer_i32_HeldItem_String name={} uuid={:?} props={} skin_url={:?} skin_model={:?}",
                    sp.name,
                    uuid,
                    sp.properties.data.len(),
                    skin_url,
                    skin_model
                );
                let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::PlayerInfoAdd {
                    uuid,
                    name: sp.name,
                    skin_url,
                    skin_model,
                }));
            }
        }
        Packet::EntityMetadata(em) => {
            handle_entity_metadata(em.entity_id.0, &em.metadata, to_main);
        }
        Packet::EntityMetadata_i32(em) => {
            handle_entity_metadata(em.entity_id, &em.metadata, to_main);
        }
        Packet::Animation(anim) => {
            let animation = match anim.animation_id {
                0 => NetEntityAnimation::SwingMainArm,
                1 => NetEntityAnimation::TakeDamage,
                2 => NetEntityAnimation::LeaveBed,
                other => NetEntityAnimation::Unknown(other),
            };
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Animation {
                entity_id: anim.entity_id.0,
                animation,
            }));
        }
        Packet::EntityProperties(_ep) => {}
        Packet::SpawnObject_i32_NoUUID(so) => {
            if object_type_to_kind(so.ty) == NetEntityKind::Item {
                debug!(
                    entity_id = so.entity_id.0,
                    data = so.data,
                    pos = ?(so.x, so.y, so.z),
                    "spawned dropped item object before metadata"
                );
            }
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: so.entity_id.0,
                uuid: None,
                kind: object_type_to_kind(so.ty),
                pos: bevy::prelude::Vec3::new(
                    f64::from(so.x) as f32,
                    f64::from(so.y) as f32,
                    f64::from(so.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(so.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(so.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnObject_i32(so) => {
            if object_type_to_kind(so.ty) == NetEntityKind::Item {
                debug!(
                    entity_id = so.entity_id.0,
                    data = so.data,
                    pos = ?(so.x, so.y, so.z),
                    "spawned dropped item object before metadata"
                );
            }
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: so.entity_id.0,
                uuid: Some(so.uuid),
                kind: object_type_to_kind(so.ty),
                pos: bevy::prelude::Vec3::new(
                    f64::from(so.x) as f32,
                    f64::from(so.y) as f32,
                    f64::from(so.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(so.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(so.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnExperienceOrb_i32(xp) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: xp.entity_id.0,
                uuid: None,
                kind: NetEntityKind::ExperienceOrb,
                pos: bevy::prelude::Vec3::new(
                    f64::from(xp.x) as f32,
                    f64::from(xp.y) as f32,
                    f64::from(xp.z) as f32,
                ),
                yaw: 0.0,
                pitch: 0.0,
                on_ground: None,
            }));
        }
        Packet::SpawnMob_u8_i32_NoUUID(sm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sm.entity_id.0,
                uuid: None,
                kind: NetEntityKind::Mob(mob_type_to_kind(sm.ty)),
                pos: bevy::prelude::Vec3::new(
                    f64::from(sm.x) as f32,
                    f64::from(sm.y) as f32,
                    f64::from(sm.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sm.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnMob_u8_i32(sm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sm.entity_id.0,
                uuid: Some(sm.uuid),
                kind: NetEntityKind::Mob(mob_type_to_kind(sm.ty)),
                pos: bevy::prelude::Vec3::new(
                    f64::from(sm.x) as f32,
                    f64::from(sm.y) as f32,
                    f64::from(sm.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sm.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnMob_u8(sm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sm.entity_id.0,
                uuid: Some(sm.uuid),
                kind: NetEntityKind::Mob(mob_type_to_kind(sm.ty)),
                pos: bevy::prelude::Vec3::new(sm.x as f32, sm.y as f32, sm.z as f32),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sm.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityHeadLook(ehl) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::HeadLook {
                entity_id: ehl.entity_id.0,
                head_yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(ehl.head_yaw)),
            }));
        }
        Packet::EntityHeadLook_i32(ehl) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::HeadLook {
                entity_id: ehl.entity_id,
                head_yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(ehl.head_yaw)),
            }));
        }
        Packet::EntityMove_i8(em) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: em.entity_id.0,
                delta: bevy::prelude::Vec3::new(
                    f64::from(em.delta_x) as f32,
                    f64::from(em.delta_y) as f32,
                    f64::from(em.delta_z) as f32,
                ),
                on_ground: Some(em.on_ground),
            }));
        }
        Packet::EntityMove_i8_i32_NoGround(em) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: em.entity_id,
                delta: bevy::prelude::Vec3::new(
                    f64::from(em.delta_x) as f32,
                    f64::from(em.delta_y) as f32,
                    f64::from(em.delta_z) as f32,
                ),
                on_ground: None,
            }));
        }
        Packet::EntityVelocity(ev) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Velocity {
                entity_id: ev.entity_id.0,
                velocity: bevy::prelude::Vec3::new(
                    ev.velocity_x as f32 / 8000.0,
                    ev.velocity_y as f32 / 8000.0,
                    ev.velocity_z as f32 / 8000.0,
                ),
            }));
        }
        Packet::EntityVelocity_i32(ev) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Velocity {
                entity_id: ev.entity_id,
                velocity: bevy::prelude::Vec3::new(
                    ev.velocity_x as f32 / 8000.0,
                    ev.velocity_y as f32 / 8000.0,
                    ev.velocity_z as f32 / 8000.0,
                ),
            }));
        }
        Packet::EntityTeleport_i32(et) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Teleport {
                entity_id: et.entity_id.0,
                pos: bevy::prelude::Vec3::new(
                    f64::from(et.x) as f32,
                    f64::from(et.y) as f32,
                    f64::from(et.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(et.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(et.pitch)),
                on_ground: Some(et.on_ground),
            }));
        }
        Packet::EntityTeleport_i32_i32_NoGround(et) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Teleport {
                entity_id: et.entity_id,
                pos: bevy::prelude::Vec3::new(
                    f64::from(et.x) as f32,
                    f64::from(et.y) as f32,
                    f64::from(et.z) as f32,
                ),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(et.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(et.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityEquipment_u16(ee) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Equipment {
                entity_id: ee.entity_id.0,
                slot: ee.slot,
                item: protocol_stack_to_inventory_item(ee.item),
            }));
        }
        Packet::EntityEquipment_u16_i32(ee) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Equipment {
                entity_id: ee.entity_id,
                slot: ee.slot,
                item: protocol_stack_to_inventory_item(ee.item),
            }));
        }
        Packet::EntityLookAndMove_i8(elm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: elm.entity_id.0,
                delta: bevy::prelude::Vec3::new(
                    f64::from(elm.delta_x) as f32,
                    f64::from(elm.delta_y) as f32,
                    f64::from(elm.delta_z) as f32,
                ),
                on_ground: Some(elm.on_ground),
            }));
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: elm.entity_id.0,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(elm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(elm.pitch)),
                on_ground: Some(elm.on_ground),
            }));
        }
        Packet::EntityLookAndMove_i8_i32_NoGround(elm) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::MoveDelta {
                entity_id: elm.entity_id,
                delta: bevy::prelude::Vec3::new(
                    f64::from(elm.delta_x) as f32,
                    f64::from(elm.delta_y) as f32,
                    f64::from(elm.delta_z) as f32,
                ),
                on_ground: None,
            }));
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: elm.entity_id,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(elm.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(elm.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityLook_VarInt(el) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: el.entity_id.0,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(el.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(el.pitch)),
                on_ground: Some(el.on_ground),
            }));
        }
        Packet::EntityLook_i32_NoGround(el) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Look {
                entity_id: el.entity_id,
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(el.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(el.pitch)),
                on_ground: None,
            }));
        }
        Packet::EntityDestroy(ed) => {
            let ids = ed.entity_ids.data.into_iter().map(|id| id.0).collect();
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Destroy {
                entity_ids: ids,
            }));
        }
        Packet::EntityDestroy_u8(ed) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Destroy {
                entity_ids: ed.entity_ids.data,
            }));
        }
        Packet::EntityStatus(es) => {
            // Map common status codes into our existing animation events.
            // 2 = hurt animation, 3 = death animation in vanilla.
            if es.entity_status == 2 {
                let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Animation {
                    entity_id: es.entity_id,
                    animation: NetEntityAnimation::TakeDamage,
                }));
            }
        }
        Packet::CollectItem_nocount(ci) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::CollectItem {
                collected_entity_id: ci.collected_entity_id.0,
                collector_entity_id: ci.collector_entity_id.0,
            }));
        }
        Packet::CollectItem_nocount_i32(ci) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::CollectItem {
                collected_entity_id: ci.collected_entity_id,
                collector_entity_id: ci.collector_entity_id,
            }));
        }
        Packet::NamedSoundEffect(sound) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: sound.name,
                position: packet_sound_position(sound.x, sound.y, sound.z),
                volume: sound.volume,
                pitch: sound.pitch,
                category_override: SoundCategory::from_vanilla_id(sound.category.0),
                distance_delay: false,
            }));
        }
        Packet::NamedSoundEffect_u8(sound) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: sound.name,
                position: packet_sound_position(sound.x, sound.y, sound.z),
                volume: sound.volume,
                pitch: f32::from(sound.pitch) / 63.0,
                category_override: SoundCategory::from_vanilla_id(sound.category.0),
                distance_delay: false,
            }));
        }
        Packet::NamedSoundEffect_u8_NoCategory(sound) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: sound.name,
                position: packet_sound_position(sound.x, sound.y, sound.z),
                volume: sound.volume,
                pitch: f32::from(sound.pitch) / 63.0,
                category_override: None,
                distance_delay: false,
            }));
        }
        Packet::Effect(effect) => {
            send_aux_sound_effect(
                to_main,
                effect.effect_id,
                bevy::prelude::Vec3::new(
                    effect.location.x as f32 + 0.5,
                    effect.location.y as f32 + 0.5,
                    effect.location.z as f32 + 0.5,
                ),
                effect.data,
            );
        }
        Packet::Effect_u8y(effect) => {
            send_aux_sound_effect(
                to_main,
                effect.effect_id,
                bevy::prelude::Vec3::new(
                    effect.x as f32 + 0.5,
                    f32::from(effect.y) + 0.5,
                    effect.z as f32 + 0.5,
                ),
                effect.data,
            );
        }
        Packet::Explosion(explosion) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: "minecraft:random.explode".to_string(),
                position: bevy::prelude::Vec3::new(explosion.x, explosion.y, explosion.z),
                volume: 4.0,
                pitch: 1.0,
                category_override: Some(SoundCategory::Block),
                distance_delay: false,
            }));
        }
        Packet::BlockChange_VarInt(bc) => {
            let update = BlockUpdate {
                x: bc.location.x,
                y: bc.location.y,
                z: bc.location.z,
                block_id: block_state_to_id_meta(bc.block_id.0),
            };
            let _ = to_main.send(FromNetMessage::BlockUpdates(vec![update]));
        }
        Packet::BlockChange_u8(bc) => {
            let update = BlockUpdate {
                x: bc.x,
                y: bc.y as i32,
                z: bc.z,
                block_id: block_state_to_id_meta(
                    (bc.block_id.0 << 4) | (bc.block_metadata as i32 & 0xF),
                ),
            };
            let _ = to_main.send(FromNetMessage::BlockUpdates(vec![update]));
        }
        Packet::MultiBlockChange_VarInt(mbc) => {
            let mut updates = Vec::with_capacity(mbc.records.data.len());
            for record in mbc.records.data {
                let local_x = (record.xz >> 4) as i32;
                let local_z = (record.xz & 0xF) as i32;
                updates.push(BlockUpdate {
                    x: mbc.chunk_x * 16 + local_x,
                    y: record.y as i32,
                    z: mbc.chunk_z * 16 + local_z,
                    block_id: block_state_to_id_meta(record.block_id.0),
                });
            }
            if !updates.is_empty() {
                let _ = to_main.send(FromNetMessage::BlockUpdates(updates));
            }
        }
        Packet::MultiBlockChange_u16(mbc) => {
            let mut updates = Vec::with_capacity(mbc.record_count as usize);
            let mut cursor = std::io::Cursor::new(mbc.data);
            use byteorder::{BigEndian, ReadBytesExt};
            for _ in 0..mbc.record_count {
                let Ok(record) = cursor.read_u32::<BigEndian>() else {
                    break;
                };
                let id_meta = (record & 0x0000_FFFF) as i32;
                let y = ((record >> 16) & 0xFF) as i32;
                let local_z = ((record >> 24) & 0x0F) as i32;
                let local_x = ((record >> 28) & 0x0F) as i32;
                updates.push(BlockUpdate {
                    x: mbc.chunk_x * 16 + local_x,
                    y,
                    z: mbc.chunk_z * 16 + local_z,
                    block_id: block_state_to_id_meta(id_meta),
                });
            }
            if !updates.is_empty() {
                let _ = to_main.send(FromNetMessage::BlockUpdates(updates));
            }
        }
        Packet::PlayerInfo(info) => {
            for detail in info.inner.players {
                match detail {
                    rs_protocol::protocol::packet::PlayerDetail::Add {
                        uuid,
                        name,
                        properties,
                        ..
                    } => {
                        let (skin_url, skin_model) =
                            extract_skin_info_from_player_properties(&properties);
                        debug!(
                            "NET PlayerInfo::Add name={} uuid={:?} props={} skin_url={:?} skin_model={:?}",
                            name,
                            uuid,
                            properties.len(),
                            skin_url,
                            skin_model
                        );
                        let _ = to_main.send(FromNetMessage::NetEntity(
                            NetEntityMessage::PlayerInfoAdd {
                                uuid,
                                name,
                                skin_url,
                                skin_model,
                            },
                        ));
                    }
                    rs_protocol::protocol::packet::PlayerDetail::Remove { uuid } => {
                        let _ = to_main.send(FromNetMessage::NetEntity(
                            NetEntityMessage::PlayerInfoRemove { uuid },
                        ));
                    }
                    _ => {}
                }
            }
        }
        Packet::UpdateBlockEntity(_ube) => {}
        Packet::KeepAliveClientbound_VarInt(ka) => {
            conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::KeepAliveServerbound_VarInt {
                    id: ka.id,
                },
            )
            .unwrap();
        }
        Packet::ServerMessage_NoPosition(sm) => {
            let text = component_to_legacy(&sm.message);
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::ServerMessage_Position(sm) => {
            let text = component_to_legacy(&sm.message);
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::ServerMessage_Sender(sm) => {
            let text = component_to_legacy(&sm.message);
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::Disconnect(disconnect) => {
            let reason = component_to_legacy(&disconnect.reason);
            let _ = to_main.send(FromNetMessage::DisconnectReason(reason));
        }
        Packet::TabCompleteReply(reply) => {
            let _ = to_main.send(FromNetMessage::TabCompleteReply(reply.matches.data));
        }
        Packet::UpdateHealth(health) => {
            let _ = to_main.send(FromNetMessage::UpdateHealth {
                health: health.health,
                food: health.food.0,
                food_saturation: health.food_saturation,
            });
        }
        Packet::UpdateHealth_u16(health) => {
            let _ = to_main.send(FromNetMessage::UpdateHealth {
                health: health.health,
                food: health.food as i32,
                food_saturation: health.food_saturation,
            });
        }
        Packet::SetExperience(exp) => {
            let _ = to_main.send(FromNetMessage::UpdateExperience {
                experience_bar: exp.experience_bar,
                level: exp.level.0,
                total_experience: exp.total_experience.0,
            });
        }
        Packet::SetExperience_i16(exp) => {
            let _ = to_main.send(FromNetMessage::UpdateExperience {
                experience_bar: exp.experience_bar,
                level: exp.level as i32,
                total_experience: exp.total_experience as i32,
            });
        }
        Packet::ChangeGameState(gs) => {
            // 1.8: reason 3 is game mode change; value stores mode as float.
            if gs.reason == 3 {
                let mode = gs.value as i32;
                if (0..=u8::MAX as i32).contains(&mode) {
                    let _ = to_main.send(FromNetMessage::GameMode {
                        gamemode: mode as u8,
                    });
                }
            }
        }
        Packet::TimeUpdate(time_update) => {
            let _ = to_main.send(FromNetMessage::TimeUpdate {
                world_age: time_update.world_age,
                time_of_day: time_update.time_of_day,
            });
        }
        Packet::Respawn_Gamemode(respawn) => {
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::Respawn_HashedSeed(respawn) => {
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::Respawn_NBT(respawn) => {
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::Respawn_WorldName(respawn) => {
            let _ = to_main.send(FromNetMessage::Respawn);
            let _ = to_main.send(FromNetMessage::GameMode {
                gamemode: respawn.gamemode,
            });
        }
        Packet::PlayerAbilities(abilities) => {
            let _ = to_main.send(FromNetMessage::PlayerAbilities {
                flags: abilities.flags,
                flying_speed: abilities.flying_speed,
                walking_speed: abilities.walking_speed,
            });
        }
        Packet::EntityEffect(effect) => {
            let _ = to_main.send(FromNetMessage::PotionEffect {
                entity_id: effect.entity_id.0,
                effect_id: effect.effect_id,
                amplifier: effect.amplifier,
                duration_ticks: effect.duration.0,
            });
        }
        Packet::EntityEffect_i32(effect) => {
            let _ = to_main.send(FromNetMessage::PotionEffect {
                entity_id: effect.entity_id,
                effect_id: effect.effect_id,
                amplifier: effect.amplifier,
                duration_ticks: i32::from(effect.duration),
            });
        }
        Packet::EntityRemoveEffect(remove) => {
            let _ = to_main.send(FromNetMessage::PotionEffectRemove {
                entity_id: remove.entity_id.0,
                effect_id: remove.effect_id,
            });
        }
        Packet::EntityRemoveEffect_i32(remove) => {
            let _ = to_main.send(FromNetMessage::PotionEffectRemove {
                entity_id: remove.entity_id,
                effect_id: remove.effect_id,
            });
        }
        Packet::WindowOpen(open) => {
            let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowOpen(
                InventoryWindowInfo {
                    id: open.id,
                    kind: open.ty,
                    title: open.title.to_string(),
                    slot_count: open.slot_count,
                },
            )));
        }
        Packet::WindowOpen_u8(open) => {
            let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowOpen(
                InventoryWindowInfo {
                    id: open.id,
                    kind: format!("type_{}", open.ty),
                    title: open.title.to_string(),
                    slot_count: open.slot_count,
                },
            )));
        }
        Packet::WindowOpen_VarInt(open) => {
            if (0..=u8::MAX as i32).contains(&open.id.0) {
                let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowOpen(
                    InventoryWindowInfo {
                        id: open.id.0 as u8,
                        kind: format!("type_{}", open.ty.0),
                        title: open.title.to_string(),
                        slot_count: 0,
                    },
                )));
            }
        }
        Packet::WindowOpenHorse(open) => {
            let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowOpen(
                InventoryWindowInfo {
                    id: open.window_id,
                    kind: "EntityHorse".to_string(),
                    title: "Horse".to_string(),
                    slot_count: open.number_of_slots.0.clamp(0, u8::MAX as i32) as u8,
                },
            )));
        }
        Packet::WindowClose(close) => {
            let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowClose {
                id: close.id,
            }));
        }
        Packet::WindowItems(items) => {
            let converted = items
                .items
                .data
                .into_iter()
                .map(protocol_stack_to_inventory_item)
                .collect();
            let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowItems {
                id: items.id,
                items: converted,
            }));
        }
        Packet::WindowSetSlot(slot) => {
            let _ = to_main.send(FromNetMessage::Inventory(InventoryMessage::WindowSetSlot {
                id: slot.id,
                slot: slot.slot,
                item: protocol_stack_to_inventory_item(slot.item),
            }));
        }
        Packet::PlayerListHeaderFooter(list_header_footer) => {
            let _ = to_main.send(FromNetMessage::TabListHeaderFooter {
                header: component_to_legacy(&list_header_footer.header),
                footer: component_to_legacy(&list_header_footer.footer),
            });
        }
        Packet::Title(title) => {
            send_title_packet(
                to_main,
                title.action.0,
                title.title.as_ref(),
                title.sub_title.as_ref(),
                title.action_bar_text.as_deref(),
                title.fade_in,
                title.fade_stay,
                title.fade_out,
            );
        }
        Packet::Title_notext(title) => match title.action.0 {
            0 => {
                if let Some(title) = title.title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetTitle {
                        text: component_to_legacy(title),
                    }));
                }
            }
            1 => {
                if let Some(subtitle) = title.sub_title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetSubtitle {
                        text: component_to_legacy(subtitle),
                    }));
                }
            }
            2 => send_title_times(to_main, title.fade_in, title.fade_stay, title.fade_out),
            3 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Clear));
            }
            4 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Reset));
            }
            _ => {}
        },
        Packet::Title_notext_component(title) => match title.action.0 {
            0 => {
                if let Some(title) = title.title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetTitle {
                        text: component_to_legacy(title),
                    }));
                }
            }
            1 => {
                if let Some(subtitle) = title.sub_title.as_ref() {
                    let _ = to_main.send(FromNetMessage::Title(TitleMessage::SetSubtitle {
                        text: component_to_legacy(subtitle),
                    }));
                }
            }
            3 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Clear));
            }
            4 => {
                let _ = to_main.send(FromNetMessage::Title(TitleMessage::Reset));
            }
            _ => {}
        },
        Packet::ScoreboardDisplay(display) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Display {
                position: display.position,
                objective_name: display.name,
            }));
        }
        Packet::ScoreboardObjective(objective) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Objective {
                name: objective.name,
                mode: Some(objective.mode),
                display_name: objective.value,
                render_type: Some(objective.ty),
            }));
        }
        Packet::ScoreboardObjective_NoMode(objective) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::Objective {
                name: objective.name,
                mode: None,
                display_name: objective.value,
                render_type: Some(objective.ty.to_string()),
            }));
        }
        Packet::UpdateScore(score) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::UpdateScore {
                entry_name: score.name,
                action: score.action,
                objective_name: score.object_name,
                value: score.value.map(|value| value.0),
            }));
        }
        Packet::UpdateScore_i32(score) => {
            let _ = to_main.send(FromNetMessage::Scoreboard(ScoreboardMessage::UpdateScore {
                entry_name: score.name,
                action: score.action,
                objective_name: score.object_name,
                value: score.value,
            }));
        }
        Packet::Teams_u8(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        Packet::Teams_NoVisColor(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        Packet::Teams_VarInt(teams) => {
            send_team_packet(
                to_main,
                teams.name,
                teams.mode,
                teams.display_name,
                teams.prefix,
                teams.suffix,
                teams.players.map(|players| players.data),
            );
        }
        Packet::ConfirmTransaction(tx) => {
            let _ = to_main.send(FromNetMessage::Inventory(
                InventoryMessage::ConfirmTransaction {
                    id: tx.id,
                    action_number: tx.action_number,
                    accepted: tx.accepted,
                },
            ));
        }
        Packet::SetCurrentHotbarSlot(slot) => {
            let _ = to_main.send(FromNetMessage::Inventory(
                InventoryMessage::SetCurrentHotbarSlot { slot: slot.slot },
            ));
        }

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
