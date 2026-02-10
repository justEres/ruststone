use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use rs_utils::{
    AppState, ApplicationState, InventoryState, PlayerStatus, ToNet, ToNetMessage, UiState,
};

pub fn hotbar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<PlayerStatus>,
    to_net: Res<ToNet>,
    mut inventory: ResMut<InventoryState>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
    {
        return;
    }

    let selected = if keys.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit4) {
        Some(3)
    } else if keys.just_pressed(KeyCode::Digit5) {
        Some(4)
    } else if keys.just_pressed(KeyCode::Digit6) {
        Some(5)
    } else if keys.just_pressed(KeyCode::Digit7) {
        Some(6)
    } else if keys.just_pressed(KeyCode::Digit8) {
        Some(7)
    } else if keys.just_pressed(KeyCode::Digit9) {
        Some(8)
    } else {
        None
    };

    let Some(slot) = selected else {
        let mut wheel_delta = 0.0f32;
        for ev in mouse_wheel_events.read() {
            wheel_delta += ev.y;
        }

        if wheel_delta.abs() < f32::EPSILON {
            return;
        }

        // Vanilla-like behavior: wheel up selects previous slot, wheel down next.
        let steps = wheel_delta.round() as i32;
        if steps == 0 {
            return;
        }
        let mut slot = inventory.selected_hotbar_slot as i32;
        slot -= steps;
        slot = slot.rem_euclid(9);
        let slot = slot as u8;

        if inventory.selected_hotbar_slot != slot {
            inventory.selected_hotbar_slot = slot;
            let _ = to_net
                .0
                .send(ToNetMessage::HeldItemChange { slot: slot as i16 });
        }
        return;
    };
    if inventory.selected_hotbar_slot != slot {
        inventory.selected_hotbar_slot = slot;
        let _ = to_net
            .0
            .send(ToNetMessage::HeldItemChange { slot: slot as i16 });
    }
}

pub fn inventory_transaction_ack_system(to_net: Res<ToNet>, mut inventory: ResMut<InventoryState>) {
    for (id, action_number) in inventory.drain_confirm_acks() {
        let _ = to_net.0.send(ToNetMessage::ConfirmTransaction {
            id,
            action_number,
            accepted: true,
        });
    }
}
