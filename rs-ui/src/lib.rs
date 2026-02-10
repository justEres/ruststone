use bevy::app::Plugin;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use bevy::window::{PresentMode, PrimaryWindow};
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self},
};
use rs_render::RenderDebugSettings;
use rs_utils::{
    AppState, ApplicationState, Chat, InventoryItemStack, InventoryState, PerfTimings,
    PlayerStatus, ToNet, ToNetMessage, UiState,
};

const INVENTORY_SLOT_SIZE: f32 = 40.0;
const INVENTORY_SLOT_SPACING: f32 = 4.0;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(EguiPrimaryContextPass, connect_ui)
            .add_plugins(EguiPlugin::default())
            .init_resource::<ConnectUiState>();
    }
}

fn connect_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<ConnectUiState>,
    app_state: Res<AppState>,
    to_net: Res<ToNet>,
    mut chat: ResMut<Chat>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<UiState>,
    mut inventory_state: ResMut<InventoryState>,
    player_status: Res<PlayerStatus>,
    mut render_debug: ResMut<RenderDebugSettings>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mut window_events: EventReader<WindowFocused>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    let ctx = contexts.ctx_mut().unwrap();

    for ev in window_events.read() {
        if ev.focused {
            ui_state.paused = false;
        } else {
            ui_state.paused = true;
        }
    }

    if keys.just_pressed(KeyCode::Escape) && ui_state.chat_open {
        ui_state.chat_open = false;
    } else if keys.just_pressed(KeyCode::Escape) && ui_state.inventory_open {
        close_open_window_if_needed(&to_net, &mut inventory_state);
        ui_state.inventory_open = false;
    } else if keys.just_pressed(KeyCode::Escape) {
        ui_state.paused = !ui_state.paused;
    } else if keys.just_pressed(KeyCode::KeyE)
        && matches!(app_state.0, ApplicationState::Connected)
        && !ui_state.paused
        && !player_status.dead
        && !ui_state.chat_open
        && !ctx.wants_keyboard_input()
    {
        if ui_state.inventory_open {
            close_open_window_if_needed(&to_net, &mut inventory_state);
            ui_state.inventory_open = false;
        } else {
            ui_state.inventory_open = true;
        }
    } else if keys.just_pressed(KeyCode::KeyT) && !ctx.wants_keyboard_input() {
        if !ui_state.inventory_open {
            ui_state.chat_open = !ui_state.chat_open;
        }
        if ui_state.chat_open {
            chat.1.clear();
        }
    }

    let show_connect_window = matches!(
        app_state.0,
        ApplicationState::Disconnected | ApplicationState::Connecting
    );
    if show_connect_window {
        ui_state.inventory_open = false;
    }

    if show_connect_window {
        egui::Window::new("Connect to Server").show(ctx, |ui| {
            ui.label("Server Address:");
            ui.text_edit_singleline(&mut state.server_address);
            ui.label("Username:");
            ui.text_edit_singleline(&mut state.username);
            if ui.button("Connect").clicked() {
                to_net
                    .0
                    .send(ToNetMessage::Connect {
                        username: state.username.clone(),
                        address: state.server_address.clone(),
                    })
                    .unwrap();
            }
            if let ApplicationState::Connecting = app_state.0 {
                ui.label("Connecting...");
            }
        });
    }

    if ui_state.chat_open {
        egui::Window::new("Chat").vscroll(true).show(ctx, |ui| {
            for msg in chat.0.iter() {
                ui.label(msg);
            }

            let response = ui.text_edit_singleline(&mut chat.1);
            response.request_focus();

            if response.has_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !chat.1.is_empty()
            {
                to_net
                    .0
                    .send(ToNetMessage::ChatMessage(chat.1.clone()))
                    .unwrap();
                chat.1.clear();
                response.request_focus();
            }
        });
    }

    if matches!(app_state.0, ApplicationState::Connected) && player_status.dead {
        egui::Window::new("You Died")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading("You Died");
                ui.add_space(8.0);
                if ui.button("Respawn").clicked() {
                    let _ = to_net.0.send(ToNetMessage::Respawn);
                }
            });
    }

    if matches!(app_state.0, ApplicationState::Connected) && ui_state.paused && !player_status.dead
    {
        egui::Window::new("Paused")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading("Game Paused");
                ui.add_space(8.0);
                ui.add(egui::Slider::new(&mut render_debug.fov_deg, 60.0..=120.0).text("FOV"));
                if ui.checkbox(&mut state.vsync_enabled, "VSync").changed() {
                    if let Ok(mut window) = window_query.get_single_mut() {
                        window.present_mode = if state.vsync_enabled {
                            PresentMode::AutoVsync
                        } else {
                            PresentMode::AutoNoVsync
                        };
                    }
                }
                ui.add_space(8.0);
                if ui.button("Video Settings (todo)").clicked() {}
                if ui.button("Controls (todo)").clicked() {}
                if ui.button("Done").clicked() {
                    ui_state.paused = false;
                }
            });
    }

    if matches!(app_state.0, ApplicationState::Connected) && !player_status.dead {
        draw_hotbar_ui(ctx, &inventory_state);
    }

    if matches!(app_state.0, ApplicationState::Connected)
        && ui_state.inventory_open
        && !ui_state.paused
        && !player_status.dead
    {
        let mut open = true;
        egui::Window::new(
            inventory_state
                .open_window
                .as_ref()
                .map(|w| w.title.as_str())
                .unwrap_or("Inventory"),
        )
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::new(0.0, -20.0))
        .show(ctx, |ui| {
            draw_inventory_grid(ui, &to_net, &keys, &mut inventory_state);
        });
        if !open {
            close_open_window_if_needed(&to_net, &mut inventory_state);
            ui_state.inventory_open = false;
        }

        draw_inventory_cursor_item(ctx, inventory_state.cursor_item);
    }

    if matches!(app_state.0, ApplicationState::Connected)
        && !ui_state.paused
        && !ui_state.chat_open
        && !ui_state.inventory_open
        && !player_status.dead
    {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("crosshair"),
        ));
        let center = ctx.screen_rect().center();
        let stroke = egui::Stroke::new(1.5, egui::Color32::from_white_alpha(230));
        let arm = 7.0;
        let gap = 2.0;

        painter.line_segment(
            [
                egui::pos2(center.x - arm, center.y),
                egui::pos2(center.x - gap, center.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(center.x + gap, center.y),
                egui::pos2(center.x + arm, center.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(center.x, center.y - arm),
                egui::pos2(center.x, center.y - gap),
            ],
            stroke,
        );
        painter.line_segment(
            [
                egui::pos2(center.x, center.y + gap),
                egui::pos2(center.x, center.y + arm),
            ],
            stroke,
        );
    }

    timings.ui_ms = start.elapsed().as_secs_f32() * 1000.0;
}

#[derive(Resource)]
pub struct ConnectUiState {
    pub username: String,
    pub server_address: String,
    pub vsync_enabled: bool,
}
impl Default for ConnectUiState {
    fn default() -> Self {
        Self {
            username: "RustyPlayer".to_string(),
            server_address: "localhost:25565".to_string(),
            vsync_enabled: false,
        }
    }
}

fn draw_hotbar_ui(ctx: &egui::Context, inventory_state: &InventoryState) {
    egui::Area::new(egui::Id::new("hotbar_overlay"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0, -12.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_black_alpha(170))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(64)))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    egui::Grid::new("hud_hotbar_grid")
                        .spacing(egui::Vec2::new(
                            INVENTORY_SLOT_SPACING,
                            INVENTORY_SLOT_SPACING,
                        ))
                        .show(ui, |ui| {
                            for hotbar_idx in 0..9u8 {
                                let item = inventory_state.hotbar_item(hotbar_idx);
                                let selected = inventory_state.selected_hotbar_slot == hotbar_idx;
                                let _ = draw_slot(ui, item, selected, INVENTORY_SLOT_SIZE, false);
                            }
                            ui.end_row();
                        });
                });
        });
}

fn draw_inventory_grid(
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    inventory_state: &mut InventoryState,
) {
    ui.label("Survival Inventory");
    ui.add_space(4.0);
    let mut hovered_any_slot = false;

    egui::Grid::new("inventory_main_grid")
        .spacing(egui::Vec2::new(
            INVENTORY_SLOT_SPACING,
            INVENTORY_SLOT_SPACING,
        ))
        .show(ui, |ui| {
            for row in 0..3usize {
                for col in 0..9usize {
                    let slot = 9 + row * 9 + col;
                    let item = inventory_state.player_slots.get(slot).copied().flatten();
                    let response = draw_slot(ui, item, false, INVENTORY_SLOT_SIZE, true);
                    if response.hovered() {
                        hovered_any_slot = true;
                    }
                    handle_inventory_slot_interaction(
                        response,
                        slot as i16,
                        keys,
                        to_net,
                        inventory_state,
                    );
                }
                ui.end_row();
            }
        });

    ui.add_space(8.0);
    ui.label("Hotbar");
    ui.add_space(4.0);
    egui::Grid::new("inventory_hotbar_grid")
        .spacing(egui::Vec2::new(
            INVENTORY_SLOT_SPACING,
            INVENTORY_SLOT_SPACING,
        ))
        .show(ui, |ui| {
            for hotbar_idx in 0..9u8 {
                let item = inventory_state.hotbar_item(hotbar_idx);
                let selected = inventory_state.selected_hotbar_slot == hotbar_idx;
                let slot = 36 + hotbar_idx as i16;
                let response = draw_slot(ui, item, selected, INVENTORY_SLOT_SIZE, true);
                if response.hovered() {
                    hovered_any_slot = true;
                }
                handle_inventory_slot_interaction(response, slot, keys, to_net, inventory_state);
            }
            ui.end_row();
        });

    let clicked_primary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
    let clicked_secondary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    if !hovered_any_slot && ui.rect_contains_pointer(ui.max_rect()) {
        if clicked_primary {
            send_inventory_click(-999, 0, 0, to_net, inventory_state);
        } else if clicked_secondary {
            send_inventory_click(-999, 1, 0, to_net, inventory_state);
        }
    }
}

fn draw_slot(
    ui: &mut egui::Ui,
    item: Option<InventoryItemStack>,
    selected: bool,
    size: f32,
    clickable: bool,
) -> egui::Response {
    let sense = if clickable {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size), sense);
    let bg = if selected {
        egui::Color32::from_gray(84)
    } else {
        egui::Color32::from_gray(44)
    };
    let stroke = if selected {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(210, 210, 210))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_gray(90))
    };

    ui.painter()
        .rect(rect, 4.0, bg, stroke, egui::StrokeKind::Outside);

    if let Some(stack) = item {
        let label = item_short_label(stack.item_id);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
        if stack.count > 1 {
            ui.painter().text(
                rect.right_bottom() - egui::vec2(4.0, 3.0),
                egui::Align2::RIGHT_BOTTOM,
                stack.count.to_string(),
                egui::FontId::proportional(11.0),
                egui::Color32::WHITE,
            );
        }
    }
    response
}

fn item_short_label(item_id: i32) -> &'static str {
    match item_id {
        1 => "Stone",
        2 => "Grass",
        3 => "Dirt",
        4 => "Cobble",
        5 => "Wood",
        12 => "Sand",
        13 => "Gravel",
        17 => "Log",
        18 => "Leaf",
        20 => "Glass",
        50 => "Torch",
        54 => "Chest",
        58 => "Craft",
        61 | 62 => "Furn",
        256 => "Shovel",
        257 => "Pick",
        258 => "Axe",
        260 => "Apple",
        261 => "Bow",
        262 => "Arrow",
        264 => "Diamond",
        267 => "Sword",
        268..=279 => "Tool",
        280 => "Stick",
        297 => "Bread",
        364 => "Steak",
        _ => "Item",
    }
}

fn draw_inventory_cursor_item(ctx: &egui::Context, cursor_item: Option<InventoryItemStack>) {
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

fn handle_inventory_slot_interaction(
    response: egui::Response,
    slot: i16,
    keys: &ButtonInput<KeyCode>,
    to_net: &ToNet,
    inventory_state: &mut InventoryState,
) {
    let shift_pressed = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl_pressed = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if response.hovered() {
        if keys.just_pressed(KeyCode::Digit1) {
            send_inventory_click(slot, 0, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit2) {
            send_inventory_click(slot, 1, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit3) {
            send_inventory_click(slot, 2, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit4) {
            send_inventory_click(slot, 3, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit5) {
            send_inventory_click(slot, 4, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit6) {
            send_inventory_click(slot, 5, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit7) {
            send_inventory_click(slot, 6, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit8) {
            send_inventory_click(slot, 7, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::Digit9) {
            send_inventory_click(slot, 8, 2, to_net, inventory_state);
            return;
        }
        if keys.just_pressed(KeyCode::KeyQ) {
            let button = if ctrl_pressed { 1 } else { 0 };
            send_inventory_click(slot, button, 4, to_net, inventory_state);
            return;
        }
    }

    if response.double_clicked_by(egui::PointerButton::Primary) {
        send_inventory_click(slot, 0, 6, to_net, inventory_state);
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
    send_inventory_click(slot, button, mode, to_net, inventory_state);
}

fn send_inventory_click(
    slot: i16,
    button: u8,
    mode: u8,
    to_net: &ToNet,
    inventory_state: &mut InventoryState,
) {
    let clicked_item = inventory_state.apply_local_click_player_window(slot, button, mode);
    let action_number = inventory_state.next_action_number;
    inventory_state.next_action_number = inventory_state.next_action_number.wrapping_add(1);
    let _ = to_net.0.send(ToNetMessage::ClickWindow {
        id: 0,
        slot,
        button,
        mode,
        action_number,
        clicked_item,
    });
}

fn close_open_window_if_needed(to_net: &ToNet, inventory_state: &mut InventoryState) {
    if let Some(window) = inventory_state.open_window.take() {
        if window.id != 0 {
            let _ = to_net.0.send(ToNetMessage::CloseWindow { id: window.id });
        }
    }
}
