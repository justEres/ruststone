use rs_protocol::protocol::Conn;
use rs_utils::{EntityUseAction, InventoryItemStack, ToNetMessage};
use tracing::{info, warn};

pub(super) fn send_session_message(conn: &mut Conn, msg: ToNetMessage) {
    match msg {
        ToNetMessage::ChatMessage(text) => {
            let sanitized = sanitize_outgoing_chat(&text);
            if sanitized != text {
                warn!(
                    "Sanitized outgoing chat (removed unsupported chars and/or clamped to 100 chars)"
                );
            }
            info!(message = %sanitized, "Outgoing chat");
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ChatMessage {
                    message: sanitized,
                },
            );
        }
        ToNetMessage::TabCompleteRequest { text } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::TabComplete_NoAssume {
                    text,
                    has_target: false,
                    target: None,
                },
            );
        }
        ToNetMessage::PlayerMovePosLook {
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerPositionLook {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    on_ground,
                },
            );
        }
        ToNetMessage::PlayerMovePos { x, y, z, on_ground } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerPosition {
                    x,
                    y,
                    z,
                    on_ground,
                },
            );
        }
        ToNetMessage::PlayerMoveLook {
            yaw,
            pitch,
            on_ground,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerLook {
                    yaw,
                    pitch,
                    on_ground,
                },
            );
        }
        ToNetMessage::PlayerMoveGround { on_ground } => {
            let _ = conn.write_packet(rs_protocol::protocol::packet::play::serverbound::Player {
                on_ground,
            });
        }
        ToNetMessage::Respawn => {
            let _ = rs_protocol::protocol::packet::send_client_status(
                conn,
                rs_protocol::protocol::packet::ClientStatus::PerformRespawn,
            );
        }
        ToNetMessage::PlayerAction {
            entity_id,
            action_id,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerAction {
                    entity_id: rs_protocol::protocol::VarInt(entity_id),
                    action_id: rs_protocol::protocol::VarInt(action_id as i32),
                    jump_boost: rs_protocol::protocol::VarInt(0),
                },
            );
        }
        ToNetMessage::ClientAbilities {
            flags,
            flying_speed,
            walking_speed,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ClientAbilities_f32 {
                    flags,
                    flying_speed,
                    walking_speed,
                },
            );
        }
        ToNetMessage::SwingArm => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ArmSwing_Handsfree { empty: () },
            );
        }
        ToNetMessage::UseEntity { target_id, action } => {
            let ty = match action {
                EntityUseAction::Interact => 0,
                EntityUseAction::Attack => 1,
            };
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::UseEntity_Handsfree {
                    target_id: rs_protocol::protocol::VarInt(target_id),
                    ty: rs_protocol::protocol::VarInt(ty),
                    target_x: 0.0,
                    target_y: 0.0,
                    target_z: 0.0,
                },
            );
        }
        ToNetMessage::HeldItemChange { slot } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::HeldItemChange { slot },
            );
        }
        ToNetMessage::ClickWindow {
            id,
            slot,
            button,
            mode,
            action_number,
            clicked_item,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ClickWindow_u8 {
                    id,
                    slot,
                    button,
                    mode,
                    action_number,
                    clicked_item: clicked_item.map(to_protocol_stack),
                },
            );
        }
        ToNetMessage::ConfirmTransaction {
            id,
            action_number,
            accepted,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ConfirmTransactionServerbound {
                    id,
                    action_number,
                    accepted,
                },
            );
        }
        ToNetMessage::CloseWindow { id } => {
            let _ = conn
                .write_packet(rs_protocol::protocol::packet::play::serverbound::CloseWindow { id });
        }
        ToNetMessage::DigStart { x, y, z, face } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                    status: 0,
                    location: rs_protocol::shared::Position::new(x, y, z),
                    face,
                },
            );
        }
        ToNetMessage::DigCancel { x, y, z, face } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                    status: 1,
                    location: rs_protocol::shared::Position::new(x, y, z),
                    face,
                },
            );
        }
        ToNetMessage::DigFinish { x, y, z, face } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                    status: 2,
                    location: rs_protocol::shared::Position::new(x, y, z),
                    face,
                },
            );
        }
        ToNetMessage::PlaceBlock {
            x,
            y,
            z,
            face,
            cursor_x,
            cursor_y,
            cursor_z,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerBlockPlacement_u8_Item {
                    location: rs_protocol::shared::Position::new(x, y, z),
                    face,
                    hand: None,
                    cursor_x,
                    cursor_y,
                    cursor_z,
                },
            );
        }
        ToNetMessage::UseItem { held_item } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerBlockPlacement_u8_Item {
                    location: rs_protocol::shared::Position::new(-1, -1, -1),
                    face: -1,
                    hand: held_item.map(to_protocol_stack),
                    cursor_x: 0,
                    cursor_y: 0,
                    cursor_z: 0,
                },
            );
        }
        ToNetMessage::DropHeldItem { full_stack } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                    status: if full_stack { 3 } else { 4 },
                    location: rs_protocol::shared::Position::new(-1, -1, -1),
                    face: 255,
                },
            );
        }
        ToNetMessage::Connect { .. } | ToNetMessage::Disconnect | ToNetMessage::Shutdown => {}
    }
}

fn sanitize_outgoing_chat(input: &str) -> String {
    let filtered: String = input
        .chars()
        .filter(|&ch| ch >= ' ' && ch != '\u{7f}' && ch != '§')
        .collect();
    if filtered.chars().count() > 100 {
        filtered.chars().take(100).collect()
    } else {
        filtered
    }
}

fn to_protocol_stack(item: InventoryItemStack) -> rs_protocol::item::Stack {
    let mut stack = rs_protocol::item::Stack::default();
    stack.id = item.item_id as isize;
    stack.count = item.count as isize;
    stack.damage = Some(item.damage as isize);
    stack
}
