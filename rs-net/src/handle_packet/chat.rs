use super::*;

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
    match pkt {
        Packet::ServerMessage_NoPosition(sm) => {
            let text = component_to_legacy(&sm.message);
            info!(message = %text, "Incoming chat");
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::ServerMessage_Position(sm) => {
            let text = component_to_legacy(&sm.message);
            info!(message = %text, "Incoming chat");
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::ServerMessage_Sender(sm) => {
            let text = component_to_legacy(&sm.message);
            info!(message = %text, "Incoming chat");
            to_main.send(FromNetMessage::ChatMessage(text)).unwrap();
        }
        Packet::Disconnect(disconnect) => {
            let reason = component_to_legacy(&disconnect.reason);
            let _ = to_main.send(FromNetMessage::DisconnectReason(reason));
        }
        Packet::TabCompleteReply(reply) => {
            let _ = to_main.send(FromNetMessage::TabCompleteReply(reply.matches.data));
        }
        Packet::PlayerListHeaderFooter(list_header_footer) => {
            let _ = to_main.send(FromNetMessage::TabListHeaderFooter {
                header: component_to_legacy(&list_header_footer.header),
                footer: component_to_legacy(&list_header_footer.footer),
            });
        }
        _ => {}
    }
}
