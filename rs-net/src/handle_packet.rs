use rs_protocol::protocol::{
    Conn,
    packet::{Packet, play::clientbound::ChunkData},
};
use rs_utils::FromNetMessage;

pub fn handle_packet(
    pkt: Packet,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    conn: &mut Conn,
) {
    use rs_protocol::protocol::packet::Packet;
    match pkt {
        Packet::ChunkData(cd) => {}
        Packet::ChunkDataBulk(cdb) => {}
        Packet::EntityMetadata(em) => {}
        Packet::EntityProperties(ep) => {}
        Packet::SpawnMob_u8_i32_NoUUID(sm) => {}
        Packet::EntityHeadLook(ehl) => {}
        Packet::EntityMove_i8(em) => {}
        Packet::EntityVelocity(ev) => {}
        Packet::EntityTeleport_i32(et) => {}
        Packet::EntityEquipment_u16(ee) => {}
        Packet::EntityLookAndMove_i8(elm) => {}
        Packet::EntityLook_VarInt(el) => {}
        Packet::UpdateBlockEntity(ube) => {}
        Packet::KeepAliveClientbound_VarInt(ka) => {
            conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::KeepAliveServerbound_VarInt {
                    id: ka.id,
                },
            )
            .unwrap();
            println!("Sent KeepAlive response");
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

        other => {
            let dbg = format!("{:?}", other);
            // Extract variant name (text before first '(') if present
            let variant = if let Some(idx) = dbg.find('(') {
                dbg[..idx].to_string()
            } else {
                dbg.clone()
            };
            println!("RECV: {}", variant);
        }
    }
}
