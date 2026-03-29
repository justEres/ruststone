use super::*;

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
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
                    warn!("Failed to decode ChunkData: {}", err);
                }
            }
        }
        Packet::ChunkData_NoEntities(cd) => {
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
                    warn!("Failed to decode ChunkData_NoEntities: {}", err);
                }
            }
        }
        Packet::ChunkData_NoEntities_u16(cd) => {
            match chunk_decode::decode_chunk(
                cd.chunk_x,
                cd.chunk_z,
                cd.new,
                cd.bitmask,
                &cd.data.data,
                true,
            ) {
                Ok((chunk, _)) => {
                    let _ = to_main.send(FromNetMessage::ChunkData(chunk));
                }
                Err(err) => {
                    warn!("Failed to decode ChunkData_NoEntities_u16: {}", err);
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
        Packet::UpdateBlockEntity(_ube) => {}
        _ => {}
    }
}
