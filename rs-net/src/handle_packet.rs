use rs_protocol::protocol::{Conn, packet::Packet};
use rs_utils::{BlockUpdate, FromNetMessage, NetEntityKind, NetEntityMessage, PlayerPosition};

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
                let _ = to_main.send(FromNetMessage::NetEntity(NetEntityMessage::PlayerInfoAdd {
                    uuid,
                    name: sp.name,
                }));
            }
        }
        Packet::EntityMetadata(_em) => {}
        Packet::EntityProperties(_ep) => {}
        Packet::SpawnMob_u8_i32_NoUUID(_sm) => {}
        Packet::EntityHeadLook(_ehl) => {}
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
        Packet::EntityVelocity(_ev) => {}
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
        Packet::EntityEquipment_u16(_ee) => {}
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
        Packet::BlockChange_VarInt(bc) => {
            let update = BlockUpdate {
                x: bc.location.x,
                y: bc.location.y,
                z: bc.location.z,
                block_id: block_state_to_id(bc.block_id.0),
            };
            let _ = to_main.send(FromNetMessage::BlockUpdates(vec![update]));
        }
        Packet::BlockChange_u8(bc) => {
            let update = BlockUpdate {
                x: bc.x,
                y: bc.y as i32,
                z: bc.z,
                block_id: block_state_to_id(
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
                    block_id: block_state_to_id(record.block_id.0),
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
                    block_id: block_state_to_id(id_meta),
                });
            }
            if !updates.is_empty() {
                let _ = to_main.send(FromNetMessage::BlockUpdates(updates));
            }
        }
        Packet::PlayerInfo(info) => {
            for detail in info.inner.players {
                match detail {
                    rs_protocol::protocol::packet::PlayerDetail::Add { uuid, name, .. } => {
                        let _ = to_main.send(FromNetMessage::NetEntity(
                            NetEntityMessage::PlayerInfoAdd { uuid, name },
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

fn block_state_to_id(block_state: i32) -> u16 {
    if block_state <= 0 {
        0
    } else {
        (block_state >> 4) as u16
    }
}
