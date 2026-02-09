use rs_protocol::protocol::{Conn, packet::Packet};
use rs_utils::{FromNetMessage, PlayerPosition};

use crate::chunk_decode;

pub fn handle_packet(
    pkt: Packet,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
) {
    use rs_protocol::protocol::packet::Packet;
    match pkt {
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
        Packet::EntityMetadata(_em) => {}
        Packet::EntityProperties(_ep) => {}
        Packet::SpawnMob_u8_i32_NoUUID(_sm) => {}
        Packet::EntityHeadLook(_ehl) => {}
        Packet::EntityMove_i8(_em) => {}
        Packet::EntityVelocity(_ev) => {}
        Packet::EntityTeleport_i32(_et) => {}
        Packet::EntityEquipment_u16(_ee) => {}
        Packet::EntityLookAndMove_i8(_elm) => {}
        Packet::EntityLook_VarInt(_el) => {}
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

        _other => {}
    }
}
