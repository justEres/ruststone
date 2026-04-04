use super::*;
use crate::debug_items::{build_debug_item_list, draw_debug_item_browser};
use crate::hud::draw_hotbar_ui;
use crate::inventory_interaction::{close_open_window_if_needed, draw_inventory_cursor_item};
use crate::inventory_ui::draw_inventory_grid;
use crate::item_icons::ItemIconCache;
use crate::options_persistence::{
    load_client_options, load_prism_accounts, save_client_options, short_uuid,
};
use crate::options_ui::render_settings_panel;
use crate::overlays::{
    alpha_to_u8, draw_action_bar_overlay, draw_chat_message, draw_scoreboard_sidebar,
    draw_tab_list_overlay, draw_title_overlay, handle_chat_tab_complete,
};
use crate::state::{ChatAutocompleteState, ConnectUiState};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(EguiPrimaryContextPass, connect_ui)
            .add_plugins(EguiPlugin::default())
            .init_resource::<ConnectUiState>()
            .init_resource::<ItemIconCache>()
            .init_resource::<ChatAutocompleteState>();
    }
}

#[derive(SystemParam)]
struct HudParams<'w, 's> {
    player_status: Res<'w, PlayerStatus>,
    world_time: Res<'w, WorldTime>,
    title_overlay: Res<'w, TitleOverlayState>,
    tab_list_header_footer: Res<'w, TabListHeaderFooter>,
    scoreboard: Res<'w, ScoreboardState>,
    break_indicator: Res<'w, BreakIndicator>,
    _marker: std::marker::PhantomData<&'s ()>,
}

fn connect_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<ConnectUiState>,
    mut app_state: ResMut<AppState>,
    to_net: Res<ToNet>,
    mut chat: ResMut<Chat>,
    mut chat_autocomplete: ResMut<ChatAutocompleteState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<UiState>,
    mut inventory_state: ResMut<InventoryState>,
    mut item_icons: ResMut<ItemIconCache>,
    mut sound_settings: ResMut<SoundSettings>,
    hud: HudParams,
    mut render_debug: ResMut<RenderDebugSettings>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mut window_events: EventReader<WindowFocused>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    let player_status = &hud.player_status;
    let world_time = &hud.world_time;
    let title_overlay = &hud.title_overlay;
    let tab_list_header_footer = &hud.tab_list_header_footer;
    let scoreboard = &hud.scoreboard;
    let break_indicator = &hud.break_indicator;

    for ev in window_events.read() {
        if ev.focused {
            ui_state.paused = false;
        } else {
            ui_state.paused = true;
        }
    }

    if !state.options_loaded {
        let options_path = state.options_path.clone();
        match window_query.single_mut() {
            Ok(mut window) => {
                match load_client_options(
                    &options_path,
                    &mut state,
                    &mut render_debug,
                    &mut sound_settings,
                    &mut window,
                ) {
                    Ok(()) => {
                        state.options_status = format!("Loaded {}", options_path);
                    }
                    Err(err) => {
                        state.options_status = err;
                    }
                }
            }
            Err(_) => {
                state.options_status = "Primary window unavailable for options load".to_string();
            }
        }
        state.options_loaded = true;
    }

    if keys.just_pressed(KeyCode::F1) {
        ui_state.ui_hidden = !ui_state.ui_hidden;
        if ui_state.ui_hidden {
            ui_state.chat_open = false;
            ui_state.inventory_open = false;
            ui_state.paused = false;
            state.debug_items_open = false;
            chat_autocomplete.clear();
        }
    }

    if ui_state.ui_hidden {
        timings.ui_ms = start.elapsed().as_secs_f32() * 1000.0;
        return;
    }

    let ctx = contexts.ctx_mut().unwrap();

    if matches!(app_state.0, ApplicationState::Connected)
        && inventory_state
            .open_window
            .as_ref()
            .is_some_and(|window| window.id != 0)
    {
        ui_state.inventory_open = true;
        ui_state.chat_open = false;
    }

    if keys.just_pressed(KeyCode::Escape) && ui_state.chat_open {
        ui_state.chat_open = false;
        chat_autocomplete.clear();
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
            chat_autocomplete.clear();
            chat_autocomplete.query_snapshot.clear();
        }
    }

    if keys.just_pressed(KeyCode::F8) && !ctx.wants_keyboard_input() {
        state.debug_items_open = !state.debug_items_open;
        if state.debug_items_open && state.debug_items.is_empty() {
            state.debug_items = build_debug_item_list();
        }
    }

    let show_connect_window = matches!(
        app_state.0,
        ApplicationState::Disconnected | ApplicationState::Connecting
    );
    if show_connect_window {
        ui_state.inventory_open = false;
        if !state.auth_accounts_loaded {
            state.auth_accounts = load_prism_accounts(&state.prism_accounts_path);
            state.selected_auth_account = 0;
            state.auth_accounts_loaded = true;
        }
    }

    if show_connect_window {
        egui::Window::new("Ruststone")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading("Connect to Server");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Server");
                    ui.text_edit_singleline(&mut state.server_address);
                });
                ui.horizontal(|ui| {
                    ui.label("Username");
                    let lock_username = matches!(state.auth_mode, AuthMode::Authenticated)
                        && !state.auth_accounts.is_empty();
                    if lock_username {
                        let selected = &state.auth_accounts[state.selected_auth_account];
                        state.username = selected.username.clone();
                    }
                    ui.add_enabled(
                        !lock_username,
                        egui::TextEdit::singleline(&mut state.username),
                    );
                });
                ui.add_space(6.0);
                ui.label("Mode");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.auth_mode, AuthMode::Offline, "Offline");
                    ui.selectable_value(
                        &mut state.auth_mode,
                        AuthMode::Authenticated,
                        "Online (Prism)",
                    );
                });
                if matches!(state.auth_mode, AuthMode::Authenticated) {
                    ui.add_space(6.0);
                    ui.label("Prism authentication");
                    ui.horizontal(|ui| {
                        ui.label("accounts.json");
                        ui.text_edit_singleline(&mut state.prism_accounts_path);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Reload Prism Accounts").clicked() {
                            state.auth_accounts = load_prism_accounts(&state.prism_accounts_path);
                            state.selected_auth_account = 0;
                        }
                    });
                    if state.auth_accounts.is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 140, 80),
                            "No Prism accounts found. Log into Prism once, then reload.",
                        );
                    } else {
                        if state.selected_auth_account >= state.auth_accounts.len() {
                            state.selected_auth_account = 0;
                        }
                        let selected = &state.auth_accounts[state.selected_auth_account];
                        egui::ComboBox::from_label("Account")
                            .selected_text(format!(
                                "{} ({})",
                                selected.username,
                                short_uuid(&selected.uuid)
                            ))
                            .show_ui(ui, |ui| {
                                let mut chosen = state.selected_auth_account;
                                for (idx, account) in state.auth_accounts.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut chosen,
                                        idx,
                                        format!(
                                            "{} ({})",
                                            account.username,
                                            short_uuid(&account.uuid)
                                        ),
                                    );
                                }
                                if chosen != state.selected_auth_account {
                                    state.selected_auth_account = chosen;
                                }
                            });
                    }
                }
                ui.add_space(8.0);
                let connect_enabled = match state.auth_mode {
                    AuthMode::Offline => true,
                    AuthMode::Authenticated => !state.auth_accounts.is_empty(),
                };
                let connect_clicked = ui
                    .add_enabled_ui(connect_enabled, |ui| {
                        ui.add_sized([220.0, 30.0], egui::Button::new("Connect"))
                    })
                    .inner
                    .clicked();
                if connect_clicked {
                    let address = state.server_address.trim().to_string();
                    if address.is_empty() {
                        state.connect_feedback = "Server address is required".into();
                    } else {
                        let connect_payload = match state.auth_mode {
                            AuthMode::Offline => {
                                let username = state.username.trim().to_string();
                                if username.is_empty() {
                                    state.connect_feedback =
                                        "Username is required in offline mode".into();
                                    None
                                } else {
                                    Some((username, None))
                                }
                            }
                            AuthMode::Authenticated => {
                                if state.auth_accounts.is_empty() {
                                    state.connect_feedback =
                                        "No Prism accounts loaded. Log into Prism once, then reload."
                                            .into();
                                    None
                                } else {
                                    let selected =
                                        state.auth_accounts.get(state.selected_auth_account);
                                    let username = selected
                                        .map(|entry| entry.username.clone())
                                        .unwrap_or_else(|| state.username.trim().to_string());
                                    let uuid = selected.map(|entry| entry.uuid.clone());
                                    Some((username, uuid))
                                }
                            }
                        };
                        if let Some((username, auth_account_uuid)) = connect_payload {
                            match to_net.0.send(ToNetMessage::Connect {
                                username,
                                address,
                                auth_mode: state.auth_mode,
                                auth_account_uuid,
                                prism_accounts_path: Some(state.prism_accounts_path.clone()),
                                requested_view_distance: render_debug
                                    .simulation_distance_chunks
                                    .clamp(2, 64) as u8,
                            }) {
                                Ok(()) => {
                                    *app_state = AppState(ApplicationState::Connecting);
                                    state.connect_feedback.clear();
                                }
                                Err(e) => {
                                    state.connect_feedback =
                                        format!("Network thread unavailable: {}", e);
                                    *app_state = AppState(ApplicationState::Disconnected);
                                }
                            }
                        }
                    }
                }
                if let ApplicationState::Connecting = app_state.0 {
                    ui.add_space(4.0);
                    ui.label("Connecting...");
                }
                if !state.connect_feedback.is_empty() {
                    ui.add_space(4.0);
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 120, 120),
                        &state.connect_feedback,
                    );
                }
            });
    }

    if matches!(app_state.0, ApplicationState::Connected) {
        let frame_ms = if timings.frame_delta_ms > 0.0 {
            timings.frame_delta_ms
        } else {
            16.6667
        };
        let fps = if frame_ms > 0.0 {
            1000.0 / frame_ms
        } else {
            0.0
        };
        let avg_1s = fps;
        let avg_10s = fps;
        let fps_color = if fps >= 120.0 {
            egui::Color32::from_rgb(125, 220, 120)
        } else if fps >= 60.0 {
            egui::Color32::from_rgb(200, 220, 120)
        } else if fps >= 30.0 {
            egui::Color32::from_rgb(230, 170, 100)
        } else {
            egui::Color32::from_rgb(230, 110, 110)
        };
        egui::Area::new(egui::Id::new("fps_overlay"))
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(12.0, 12.0))
            .interactable(false)
            .show(ctx, |ui| {
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(96))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0);
                frame.show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("FPS: {:.0}", fps))
                            .color(fps_color)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("1s avg: {:.1}", avg_1s))
                            .color(egui::Color32::from_gray(220)),
                    );
                    ui.label(
                        egui::RichText::new(format!("10s avg: {:.1}", avg_10s))
                            .color(egui::Color32::from_gray(220)),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "Time: {}",
                            world_time.time_of_day.rem_euclid(24_000)
                        ))
                        .color(egui::Color32::from_gray(220)),
                    );
                });
            });

        let panel_width = (ctx.screen_rect().width() * 0.45).clamp(280.0, 520.0);
        let visible_lines = if ui_state.chat_open { 16 } else { 8 };
        egui::Area::new(egui::Id::new("chat_overlay"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -16.0))
            .interactable(ui_state.chat_open)
            .show(ctx, |ui| {
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(alpha_to_u8(
                        state.chat_background_opacity,
                    )))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0);
                frame.show(ui, |ui| {
                    ui.set_width(panel_width);
                    let start = chat.0.len().saturating_sub(visible_lines);
                    for msg in chat.0.iter().skip(start) {
                        draw_chat_message(ui, msg, state.chat_font_size);
                    }
                    if ui_state.chat_open {
                        ui.add_space(4.0);
                        if chat.1 != chat_autocomplete.query_snapshot {
                            if chat_autocomplete.suppress_next_clear {
                                chat_autocomplete.suppress_next_clear = false;
                            } else {
                                chat_autocomplete.clear();
                            }
                            chat_autocomplete.query_snapshot = chat.1.clone();
                        }
                        let response = ui.add_sized(
                            [panel_width - 8.0, 22.0],
                            egui::TextEdit::singleline(&mut chat.1).hint_text("Type message..."),
                        );
                        response.request_focus();
                        if response.has_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Tab))
                            && !chat.1.is_empty()
                            && chat.1.starts_with('/')
                        {
                            handle_chat_tab_complete(&to_net, &mut chat, &mut chat_autocomplete);
                            response.request_focus();
                        }
                        if response.has_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !chat.1.is_empty()
                        {
                            let _ = to_net.0.send(ToNetMessage::ChatMessage(chat.1.clone()));
                            chat.1.clear();
                            chat_autocomplete.clear();
                            chat_autocomplete.query_snapshot.clear();
                            response.request_focus();
                        }
                        if ui_state.chat_open && !chat_autocomplete.suggestions.is_empty() {
                            ui.add_space(4.0);
                            let max_suggestions = 5usize;
                            for (idx, candidate) in chat_autocomplete
                                .suggestions
                                .iter()
                                .take(max_suggestions)
                                .enumerate()
                            {
                                let prefix = if idx == chat_autocomplete.selected {
                                    "> "
                                } else {
                                    "  "
                                };
                                ui.label(
                                    egui::RichText::new(format!("{}{}", prefix, candidate))
                                        .color(egui::Color32::from_gray(210)),
                                );
                            }
                        }
                    }
                });
            });

        draw_scoreboard_sidebar(ctx, &scoreboard, &state);
        draw_title_overlay(ctx, &title_overlay, &state);
        draw_action_bar_overlay(ctx, &title_overlay, &state);

        if keys.pressed(KeyCode::Tab) {
            draw_tab_list_overlay(ctx, &tab_list_header_footer, &state);
        }
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
                let mut primary_window = window_query.single_mut().ok();
                let options_changed = render_settings_panel(
                    ui,
                    &mut state,
                    &mut render_debug,
                    &mut sound_settings,
                    &mut primary_window,
                );

                if options_changed {
                    state.options_dirty = true;
                    match save_client_options(
                        &state.options_path,
                        &state,
                        &render_debug,
                        &sound_settings,
                    ) {
                        Ok(()) => {
                            state.options_status = format!("Saved {}", state.options_path);
                            state.options_dirty = false;
                        }
                        Err(err) => state.options_status = err,
                    }
                }
                ui.add_space(8.0);
                if ui.button("Controls (todo)").clicked() {}
                if ui.button("Disconnect").clicked() {
                    if state.options_dirty {
                        let _ = save_client_options(
                            &state.options_path,
                            &state,
                            &render_debug,
                            &sound_settings,
                        );
                        state.options_dirty = false;
                    }
                    let _ = to_net.0.send(ToNetMessage::Disconnect);
                    close_open_window_if_needed(&to_net, &mut inventory_state);
                    ui_state.chat_open = false;
                    ui_state.inventory_open = false;
                    ui_state.paused = false;
                    *app_state = AppState(ApplicationState::Disconnected);
                }
                if ui.button("Done").clicked() {
                    if state.options_dirty {
                        let _ = save_client_options(
                            &state.options_path,
                            &state,
                            &render_debug,
                            &sound_settings,
                        );
                        state.options_dirty = false;
                    }
                    ui_state.paused = false;
                }
            });
    }

    if matches!(app_state.0, ApplicationState::Connected) && !player_status.dead {
        draw_hotbar_ui(ctx, &inventory_state, &player_status, &mut item_icons);
    }

    if state.debug_items_open {
        draw_debug_item_browser(ctx, &mut state, &mut item_icons, &to_net);
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
            draw_inventory_grid(
                ctx,
                ui,
                &to_net,
                &keys,
                &mut state,
                &mut inventory_state,
                &mut item_icons,
            );
        });
        if !open {
            close_open_window_if_needed(&to_net, &mut inventory_state);
            ui_state.inventory_open = false;
        }

        draw_inventory_cursor_item(ctx, inventory_state.cursor_item.clone(), &mut item_icons);
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

        if break_indicator.active {
            let bar_w = 100.0;
            let bar_h = 6.0;
            let y = center.y + arm + 10.0;
            let bg_rect =
                egui::Rect::from_center_size(egui::pos2(center.x, y), egui::vec2(bar_w, bar_h));
            painter.rect(
                bg_rect,
                2.0,
                egui::Color32::from_black_alpha(170),
                egui::Stroke::new(1.0, egui::Color32::from_gray(110)),
                egui::StrokeKind::Outside,
            );
            let fill_w = (bar_w - 2.0) * break_indicator.progress.clamp(0.0, 1.0);
            if fill_w > 0.0 {
                let fill_rect = egui::Rect::from_min_size(
                    bg_rect.min + egui::vec2(1.0, 1.0),
                    egui::vec2(fill_w, bar_h - 2.0),
                );
                painter.rect_filled(fill_rect, 1.0, egui::Color32::from_rgb(210, 210, 210));
            }
        }
    }

    timings.ui_ms = start.elapsed().as_secs_f32() * 1000.0;
}
