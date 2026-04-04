use super::*;
use crate::item_icons::ItemIconCache;
use crate::tooltips::item_short_label;

pub(crate) fn draw_inventory_cursor_item(
    ctx: &egui::Context,
    cursor_item: Option<InventoryItemStack>,
    item_icons: &mut ItemIconCache,
) {
    let Some(stack) = cursor_item else {
        return;
    };
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };

    let rect = egui::Rect::from_min_size(pos + egui::vec2(10.0, 10.0), egui::vec2(56.0, 20.0));
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("inventory_cursor_item"),
    ));
    painter.rect(
        rect,
        3.0,
        egui::Color32::from_black_alpha(190),
        egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
        egui::StrokeKind::Outside,
    );
    if let Some(texture_id) = item_icons.texture_for_stack(ctx, &stack) {
        let icon_rect = egui::Rect::from_min_size(
            rect.left_top() + egui::vec2(2.0, 2.0),
            egui::vec2(16.0, 16.0),
        );
        painter.image(
            texture_id,
            icon_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        let text = if stack.count > 1 {
            format!("x{}", stack.count)
        } else {
            item_short_label(stack.item_id).to_string()
        };
        painter.text(
            rect.left_center() + egui::vec2(22.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    } else {
        let text = if stack.count > 1 {
            format!("{} x{}", item_short_label(stack.item_id), stack.count)
        } else {
            item_short_label(stack.item_id).to_string()
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }
}

pub(crate) fn handle_inventory_slot_interaction(
    ctx: &egui::Context,
    response: egui::Response,
    window_id: u8,
    window_unique_slots: usize,
    slot: i16,
    keys: &ButtonInput<KeyCode>,
    to_net: &ToNet,
    state: &mut ConnectUiState,
    inventory_state: &mut InventoryState,
) {
    let shift_pressed = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl_pressed = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if response.hovered() {
        if keys.just_pressed(KeyCode::Digit1) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                0,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit2) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                1,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit3) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                2,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit4) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                3,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit5) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                4,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit6) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                5,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit7) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                6,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit8) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                7,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::Digit9) {
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                8,
                2,
                to_net,
                inventory_state,
            );
            return;
        }
        if keys.just_pressed(KeyCode::KeyQ) {
            let button = if ctrl_pressed { 1 } else { 0 };
            send_inventory_click(
                window_id,
                window_unique_slots,
                slot,
                button,
                4,
                to_net,
                inventory_state,
            );
            return;
        }
    }

    if inventory_state.cursor_item.is_some()
        && handle_inventory_drag_interaction(
            ctx,
            &response,
            window_id,
            window_unique_slots,
            slot,
            to_net,
            state,
            inventory_state,
        )
    {
        return;
    }

    if state.inventory_drag.is_some() {
        return;
    }

    if response.double_clicked_by(egui::PointerButton::Primary) {
        send_inventory_click(
            window_id,
            window_unique_slots,
            slot,
            0,
            6,
            to_net,
            inventory_state,
        );
        return;
    }

    let click = if response.clicked_by(egui::PointerButton::Primary) {
        Some(0u8)
    } else if response.clicked_by(egui::PointerButton::Secondary) {
        Some(1u8)
    } else {
        None
    };
    let Some(button) = click else {
        return;
    };
    let mode = if shift_pressed { 1 } else { 0 };
    send_inventory_click(
        window_id,
        window_unique_slots,
        slot,
        button,
        mode,
        to_net,
        inventory_state,
    );
}

fn handle_inventory_drag_interaction(
    ctx: &egui::Context,
    response: &egui::Response,
    window_id: u8,
    window_unique_slots: usize,
    slot: i16,
    to_net: &ToNet,
    state: &mut ConnectUiState,
    inventory_state: &mut InventoryState,
) -> bool {
    let primary_drag =
        response.drag_started_by(egui::PointerButton::Primary) || response.dragged_by(egui::PointerButton::Primary);
    let secondary_drag = response.drag_started_by(egui::PointerButton::Secondary)
        || response.dragged_by(egui::PointerButton::Secondary);

    let Some((button, pointer_button)) = (if primary_drag {
        Some((0u8, egui::PointerButton::Primary))
    } else if secondary_drag {
        Some((1u8, egui::PointerButton::Secondary))
    } else {
        None
    }) else {
        return false;
    };

    if state.inventory_drag.is_none() {
        send_inventory_click(
            window_id,
            window_unique_slots,
            -999,
            drag_start_button(button),
            5,
            to_net,
            inventory_state,
        );
        state.inventory_drag = Some(InventoryDragUiState {
            window_id,
            window_unique_slots,
            button,
            visited_slots: Vec::new(),
        });
    }

    let Some(drag) = state.inventory_drag.as_mut() else {
        return false;
    };
    if drag.window_id != window_id
        || drag.window_unique_slots != window_unique_slots
        || drag.button != button
        || !ctx.input(|i| i.pointer.button_down(pointer_button))
    {
        return false;
    }

    if response.hovered() && !drag.visited_slots.contains(&slot) {
        send_inventory_click(
            window_id,
            window_unique_slots,
            slot,
            drag_add_button(button),
            5,
            to_net,
            inventory_state,
        );
        drag.visited_slots.push(slot);
    }
    true
}

pub(crate) fn finish_inventory_drag_if_released(
    ctx: &egui::Context,
    to_net: &ToNet,
    state: &mut ConnectUiState,
    inventory_state: &mut InventoryState,
) {
    let Some(drag) = state.inventory_drag.as_ref() else {
        return;
    };
    let pointer_down = ctx.input(|i| {
        i.pointer.button_down(match drag.button {
            0 => egui::PointerButton::Primary,
            _ => egui::PointerButton::Secondary,
        })
    });
    if pointer_down {
        return;
    }

    let drag = state.inventory_drag.take().unwrap();
    send_inventory_click(
        drag.window_id,
        drag.window_unique_slots,
        -999,
        drag_end_button(drag.button),
        5,
        to_net,
        inventory_state,
    );
}

fn drag_start_button(button: u8) -> u8 {
    if button == 0 { 0 } else { 4 }
}

fn drag_add_button(button: u8) -> u8 {
    if button == 0 { 1 } else { 5 }
}

fn drag_end_button(button: u8) -> u8 {
    if button == 0 { 2 } else { 6 }
}

pub(crate) fn send_inventory_click(
    window_id: u8,
    window_unique_slots: usize,
    slot: i16,
    button: u8,
    mode: u8,
    to_net: &ToNet,
    inventory_state: &mut InventoryState,
) {
    let clicked_item = inventory_state.apply_local_click_window(
        window_id,
        window_unique_slots,
        slot,
        button,
        mode,
    );
    let action_number = inventory_state.next_action_number;
    inventory_state.next_action_number = inventory_state.next_action_number.wrapping_add(1);
    let _ = to_net.0.send(ToNetMessage::ClickWindow {
        id: window_id,
        slot,
        button,
        mode,
        action_number,
        clicked_item,
    });
}

pub(crate) fn close_open_window_if_needed(to_net: &ToNet, inventory_state: &mut InventoryState) {
    if let Some(window) = inventory_state.open_window.take() {
        if window.id != 0 {
            let _ = to_net.0.send(ToNetMessage::CloseWindow { id: window.id });
        }
    }
}
