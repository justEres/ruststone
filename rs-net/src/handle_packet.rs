use base64::Engine;
use rs_protocol::protocol::{Conn, packet::Packet};
use rs_protocol::types::Value as MetadataValue;
use rs_utils::{
    BlockUpdate, FromNetMessage, InventoryEnchantment, InventoryItemMeta, InventoryItemStack,
    InventoryMessage, InventoryWindowInfo, MobKind, NetEntityAnimation, NetEntityKind,
    NetEntityMessage, ObjectKind, PlayerPosition, PlayerSkinModel, item_name,
};

use crate::chunk_decode;

pub fn handle_packet(
    pkt: Packet,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
) {
    use rs_protocol::protocol::packet::Packet;
    match pkt {
        Packet::JoinGame_i8(jg) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::LocalPlayerId {
                entity_id: jg.entity_id,
            }));
        }
        Packet::JoinGame_i8_NoDebug(jg) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::LocalPlayerId {
                entity_id: jg.entity_id,
            }));
        }
        Packet::JoinGame_i32(jg) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::LocalPlayerId {
                entity_id: jg.entity_id,
            }));
        }
        Packet::JoinGame_i32_ViewDistance(jg) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::LocalPlayerId {
                entity_id: jg.entity_id,
            }));
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
                    println!("Failed to decode ChunkData: {}", err);
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
                        println!("Failed to decode ChunkDataBulk: {}", err);
                        break;
                    }
                }
            }
        }
        Packet::TeleportPlayer_NoConfirm(tp) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((tp.x, tp.y, tp.z)),
                yaw: Some(tp.yaw),
                pitch: Some(tp.pitch),
                flags: Some(tp.flags),
                on_ground: None,
            }));
        }
        Packet::TeleportPlayer_WithConfirm(tp) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((tp.x, tp.y, tp.z)),
                yaw: Some(tp.yaw),
                pitch: Some(tp.pitch),
                flags: Some(tp.flags),
                on_ground: None,
            }));
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::TeleportConfirm {
                    teleport_id: tp.teleport_id,
                },
            );
        }
        Packet::TeleportPlayer_OnGround(tp) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((tp.x, tp.eyes_y, tp.z)),
                yaw: Some(tp.yaw),
                pitch: Some(tp.pitch),
                flags: None,
                on_ground: Some(tp.on_ground),
            }));
        }
        Packet::PlayerPosition(position) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((position.x, position.y, position.z)),
                yaw: None,
                pitch: None,
                flags: None,
                on_ground: Some(position.on_ground),
            }));
        }
        Packet::PlayerPosition_HeadY(position) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((position.x, position.feet_y, position.z)),
                yaw: None,
                pitch: None,
                flags: None,
                on_ground: Some(position.on_ground),
            }));
        }
        Packet::PlayerPositionLook(position) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((position.x, position.y, position.z)),
                yaw: Some(position.yaw),
                pitch: Some(position.pitch),
                flags: None,
                on_ground: Some(position.on_ground),
            }));
        }
        Packet::PlayerPositionLook_HeadY(position) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: Some((position.x, position.feet_y, position.z)),
                yaw: Some(position.yaw),
                pitch: Some(position.pitch),
                flags: None,
                on_ground: Some(position.on_ground),
            }));
        }
        Packet::PlayerLook(position) => {
            let _ = to_main.send(FromNetMessage::PlayerPosition(PlayerPosition {
                position: None,
                yaw: Some(position.yaw),
                pitch: Some(position.pitch),
                flags: None,
                on_ground: Some(position.on_ground),
            }));
        }
        Packet::SpawnPlayer_i32_HeldItem(sp) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sp.entity_id.0,
                uuid: Some(sp.uuid),
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
        }
        Packet::SpawnPlayer_i32(sp) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sp.entity_id.0,
                uuid: Some(sp.uuid),
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
        }
        Packet::SpawnPlayer_f64(sp) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sp.entity_id.0,
                uuid: Some(sp.uuid),
                kind: NetEntityKind::Player,
                pos: bevy::prelude::Vec3::new(sp.x as f32, sp.y as f32, sp.z as f32),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sp.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sp.pitch)),
                on_ground: None,
            }));
        }
        Packet::SpawnPlayer_f64_NoMeta(sp) => {
            let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::Spawn {
                entity_id: sp.entity_id.0,
                uuid: Some(sp.uuid),
                kind: NetEntityKind::Player,
                pos: bevy::prelude::Vec3::new(sp.x as f32, sp.y as f32, sp.z as f32),
                yaw: server_yaw_to_client_yaw(angle_i8_to_degrees(sp.yaw)),
                pitch: server_pitch_to_client_pitch(angle_i8_to_degrees(sp.pitch)),
                on_ground: None,
            }));
        }
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
                println!(
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
                        println!(
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
            let text = sm.message.to_string();
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::ServerMessage_Position(sm) => {
            let text = sm.message.to_string();
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::ServerMessage_Sender(sm) => {
            let text = sm.message.to_string();
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
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
