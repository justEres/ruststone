use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use rs_utils::{AppState, ApplicationState, UiState};

#[derive(Resource, Default)]
pub struct PlayerInput {
    pub move_axis: Vec3,
    pub look_delta: Vec2,
    pub wants_sprint: bool,
}

pub fn collect_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut motion_events: EventReader<MouseMotion>,
    mut input: ResMut<PlayerInput>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.inventory_open
        || ui_state.paused
    {
        *input = PlayerInput::default();
        motion_events.clear();
        return;
    }

    let mut look_delta = Vec2::ZERO;
    for ev in motion_events.read() {
        look_delta += ev.delta;
    }

    let mut axis = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        axis.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        axis.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        axis.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        axis.x -= 1.0;
    }
    if keys.pressed(KeyCode::Space) {
        axis.y += 1.0;
    }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        axis.y -= 1.0;
    }

    input.move_axis = axis;
    input.look_delta = look_delta;
    input.wants_sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
}

pub fn apply_cursor_lock(
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Some(mut window) = windows.iter_mut().next() else {
        return;
    };

    let should_lock = matches!(app_state.0, ApplicationState::Connected)
        && !ui_state.chat_open
        && !ui_state.paused
        && !ui_state.inventory_open;

    if should_lock {
        if window.cursor_options.grab_mode != CursorGrabMode::Locked {
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
        }
        if window.cursor_options.visible {
            window.cursor_options.visible = false;
        }
    } else {
        if window.cursor_options.grab_mode != CursorGrabMode::None {
            window.cursor_options.grab_mode = CursorGrabMode::None;
        }
        if !window.cursor_options.visible {
            window.cursor_options.visible = true;
        }
    }
}
