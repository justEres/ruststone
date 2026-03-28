use super::*;

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
    match pkt {
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
        _ => {}
    }
}
