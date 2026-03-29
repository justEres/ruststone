use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use rs_utils::{
    AppState, ApplicationState, InventoryState, PlayerStatus, SoundCategory, SoundEvent,
    SoundEventQueue, ToNet, ToNetMessage, UiState,
};

const HOTBAR_DIGIT_KEYS: [(KeyCode, u8); 9] = [
    (KeyCode::Digit1, 0),
    (KeyCode::Digit2, 1),
    (KeyCode::Digit3, 2),
    (KeyCode::Digit4, 3),
    (KeyCode::Digit5, 4),
    (KeyCode::Digit6, 5),
    (KeyCode::Digit7, 6),
    (KeyCode::Digit8, 7),
    (KeyCode::Digit9, 8),
];

pub fn hotbar_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<PlayerStatus>,
    to_net: Res<ToNet>,
    mut inventory: ResMut<InventoryState>,
    mut sound_queue: ResMut<SoundEventQueue>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
    {
        return;
    }

    if keys.just_pressed(KeyCode::KeyQ) {
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        let _ = inventory.predict_drop_selected_hotbar(ctrl);
        let _ = to_net
            .0
            .send(ToNetMessage::DropHeldItem { full_stack: ctrl });
        return;
    }

    // If zoom is active, keep hotbar stable and let wheel events drive zoom instead.
    if keys.pressed(KeyCode::KeyC) {
        for _ in mouse_wheel_events.read() {}
        // Number keys should still work while zooming, so don't early-return before that logic.
    }

    let selected = HOTBAR_DIGIT_KEYS
        .iter()
        .find_map(|(key, slot)| keys.just_pressed(*key).then_some(*slot));

    let Some(slot) = selected else {
        if keys.pressed(KeyCode::KeyC) {
            return;
        }
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
            inventory.set_selected_hotbar_slot(slot);
            let _ = to_net
                .0
                .send(ToNetMessage::HeldItemChange { slot: slot as i16 });
            sound_queue.push(SoundEvent::Ui {
                event_id: "minecraft:random.click".to_string(),
                volume: 0.25,
                pitch: 1.0,
                category_override: Some(SoundCategory::Player),
            });
        }
        return;
    };
    if inventory.selected_hotbar_slot != slot {
        inventory.set_selected_hotbar_slot(slot);
        let _ = to_net
            .0
            .send(ToNetMessage::HeldItemChange { slot: slot as i16 });
        sound_queue.push(SoundEvent::Ui {
            event_id: "minecraft:random.click".to_string(),
            volume: 0.25,
            pitch: 1.0,
            category_override: Some(SoundCategory::Player),
        });
    }
}
