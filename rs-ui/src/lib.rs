use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::app::Plugin;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use bevy::window::{PresentMode, PrimaryWindow};
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self},
};
use rs_render::{
    AntiAliasingMode, BlockModelResolver, IconQuad, LightingQualityPreset, RenderDebugSettings,
    ModelFace, ShadowQualityPreset, default_model_roots,
};
use rs_utils::{
    AppState, ApplicationState, AuthMode, BreakIndicator, Chat, InventoryItemStack, InventoryState,
    InventoryWindowInfo, PerfTimings, PlayerStatus, ToNet, ToNetMessage, UiState,
    BlockFace, block_registry_key, block_texture_name, item_max_durability, item_name,
    item_registry_key, item_texture_candidates,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

const INVENTORY_SLOT_SIZE: f32 = 40.0;
const INVENTORY_SLOT_SPACING: f32 = 4.0;
const DEFAULT_OPTIONS_PATH: &str = "ruststone_options.toml";
const DEBUG_ITEM_CELL: f32 = 52.0;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(EguiPrimaryContextPass, connect_ui)
            .add_plugins(EguiPlugin::default())
            .init_resource::<ConnectUiState>()
            .init_resource::<ItemIconCache>();
    }
}

fn connect_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<ConnectUiState>,
    mut app_state: ResMut<AppState>,
    to_net: Res<ToNet>,
    mut chat: ResMut<Chat>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<UiState>,
    mut inventory_state: ResMut<InventoryState>,
    mut item_icons: ResMut<ItemIconCache>,
    player_status: Res<PlayerStatus>,
    break_indicator: Res<BreakIndicator>,
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

    if !state.options_loaded {
        let options_path = state.options_path.clone();
        match window_query.get_single_mut() {
            Ok(mut window) => {
                match load_client_options(&options_path, &mut state, &mut render_debug, &mut window)
                {
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
        let panel_width = (ctx.screen_rect().width() * 0.45).clamp(280.0, 520.0);
        let visible_lines = if ui_state.chat_open { 16 } else { 8 };
        egui::Area::new(egui::Id::new("chat_overlay"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -16.0))
            .interactable(ui_state.chat_open)
            .show(ctx, |ui| {
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(96))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0);
                frame.show(ui, |ui| {
                    ui.set_width(panel_width);
                    let start = chat.0.len().saturating_sub(visible_lines);
                    for msg in chat.0.iter().skip(start) {
                        draw_chat_message(ui, msg);
                    }
                    if ui_state.chat_open {
                        ui.add_space(4.0);
                        let response = ui.add_sized(
                            [panel_width - 8.0, 22.0],
                            egui::TextEdit::singleline(&mut chat.1).hint_text("Type message..."),
                        );
                        response.request_focus();
                        if response.has_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !chat.1.is_empty()
                        {
                            let _ = to_net.0.send(ToNetMessage::ChatMessage(chat.1.clone()));
                            chat.1.clear();
                            response.request_focus();
                        }
                    }
                });
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
                let mut options_changed = false;
                ui.heading("Game Paused");
                ui.add_space(8.0);
                let general_section = egui::CollapsingHeader::new("General")
                    .default_open(state.options_section_general)
                    .show(ui, |ui| {
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.fov_deg, 60.0..=140.0)
                                    .text("FOV"),
                            )
                            .changed();
                        let mut render_distance = render_debug.render_distance_chunks;
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_distance, 2..=32)
                                    .text("Render Distance"),
                            )
                            .changed();
                        render_debug.render_distance_chunks = render_distance;
                        let mut selected_aa_mode = render_debug.aa_mode;
                        egui::ComboBox::from_label("Anti-aliasing")
                            .selected_text(selected_aa_mode.label())
                            .show_ui(ui, |ui| {
                                for mode in AntiAliasingMode::ALL {
                                    ui.selectable_value(&mut selected_aa_mode, mode, mode.label());
                                }
                            });
                        if selected_aa_mode != render_debug.aa_mode {
                            render_debug.aa_mode = selected_aa_mode;
                            render_debug.fxaa_enabled = matches!(
                                render_debug.aa_mode,
                                AntiAliasingMode::Fxaa
                                    | AntiAliasingMode::Msaa4
                                    | AntiAliasingMode::Msaa8
                            );
                            options_changed = true;
                        }
                        options_changed |= ui
                            .checkbox(&mut render_debug.manual_frustum_cull, "Manual frustum cull")
                            .changed();
                        if ui.checkbox(&mut state.vsync_enabled, "VSync").changed() {
                            options_changed = true;
                            if let Ok(mut window) = window_query.get_single_mut() {
                                window.present_mode = if state.vsync_enabled {
                                    PresentMode::AutoVsync
                                } else {
                                    PresentMode::AutoNoVsync
                                };
                            }
                        }
                    });
                state.options_section_general = general_section.fully_open();

                let lighting_section = egui::CollapsingHeader::new("Lighting & Shadows")
                    .default_open(state.options_section_lighting)
                    .show(ui, |ui| {
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.enable_pbr_terrain_lighting,
                                "PBR terrain path",
                            )
                            .changed();
                        options_changed |= ui
                            .checkbox(&mut render_debug.shadows_enabled, "Shadows")
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.shader_quality_mode, 0..=3)
                                    .text("Shader quality mode"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.shadow_distance_scale,
                                    0.25..=20.0,
                                )
                                .logarithmic(true)
                                .text("Shadow distance"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.shadow_map_size, 256..=4096)
                                    .text("Shadow map size"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.shadow_cascades, 1..=4)
                                    .text("Shadow cascades"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.shadow_max_distance,
                                    16.0..=320.0,
                                )
                                .text("Shadow max distance"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.sun_azimuth_deg,
                                    -180.0..=180.0,
                                )
                                .text("Sun azimuth"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.sun_elevation_deg,
                                    -20.0..=89.0,
                                )
                                .text("Sun elevation"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.sun_strength, 0.0..=2.0)
                                    .text("Sun strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.ambient_strength, 0.0..=2.0)
                                    .text("Ambient strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .checkbox(&mut render_debug.voxel_ao_enabled, "Voxel AO")
                            .changed();
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.voxel_ao_cutout,
                                "Voxel AO on cutout blocks",
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.voxel_ao_strength, 0.0..=1.0)
                                    .text("Voxel AO strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.fog_density, 0.0..=0.08)
                                    .text("Fog density"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.fog_start, 0.0..=400.0)
                                    .text("Fog start"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.fog_end, 1.0..=600.0)
                                    .text("Fog end"),
                            )
                            .changed();
                    });
                state.options_section_lighting = lighting_section.fully_open();

                let water_section = egui::CollapsingHeader::new("Water")
                    .default_open(state.options_section_water)
                    .show(ui, |ui| {
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.water_reflections_enabled,
                                "Water reflections",
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_strength,
                                    0.0..=3.0,
                                )
                                .text("Water reflection strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_near_boost,
                                    0.0..=1.0,
                                )
                                .text("Near reflection boost"),
                            )
                            .changed();
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.water_reflection_blue_tint,
                                "Blue reflection tint",
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_tint_strength,
                                    0.0..=2.0,
                                )
                                .text("Blue tint strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.water_wave_strength, 0.0..=1.2)
                                    .text("Water wave strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(&mut render_debug.water_wave_speed, 0.0..=3.0)
                                    .text("Water wave speed"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_wave_detail_strength,
                                    0.0..=1.0,
                                )
                                .text("Water detail wave strength"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_wave_detail_scale,
                                    1.0..=8.0,
                                )
                                .text("Water detail wave scale"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_wave_detail_speed,
                                    0.0..=4.0,
                                )
                                .text("Water detail wave speed"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_edge_fade,
                                    0.01..=0.5,
                                )
                                .text("Reflection edge fade"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_sky_fill,
                                    0.0..=1.0,
                                )
                                .text("Reflection sky fallback"),
                            )
                            .changed();
                        options_changed |= ui
                            .add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_overscan,
                                    1.0..=3.0,
                                )
                                .text("Reflection overscan"),
                            )
                            .changed();
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.water_reflection_screen_space,
                                "SSR reflections",
                            )
                            .changed();
                        if render_debug.water_reflection_screen_space {
                            options_changed |= ui
                                .add(
                                    egui::Slider::new(&mut render_debug.water_ssr_steps, 4..=64)
                                        .text("SSR ray steps"),
                                )
                                .changed();
                            options_changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut render_debug.water_ssr_thickness,
                                        0.02..=2.0,
                                    )
                                    .text("SSR hit thickness"),
                                )
                                .changed();
                            options_changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut render_debug.water_ssr_max_distance,
                                        4.0..=400.0,
                                    )
                                    .text("SSR max distance"),
                                )
                                .changed();
                            options_changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut render_debug.water_ssr_stride,
                                        0.2..=8.0,
                                    )
                                    .text("SSR step stride"),
                                )
                                .changed();
                        }
                    });
                state.options_section_water = water_section.fully_open();

                let layers_section = egui::CollapsingHeader::new("Render Layers")
                    .default_open(state.options_section_layers)
                    .show(ui, |ui| {
                        options_changed |= ui
                            .checkbox(&mut render_debug.show_layer_entities, "Show entities layer")
                            .changed();
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.show_layer_chunks_opaque,
                                "Show opaque chunks layer",
                            )
                            .changed();
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.show_layer_chunks_cutout,
                                "Show cutout chunks layer",
                            )
                            .changed();
                        options_changed |= ui
                            .checkbox(
                                &mut render_debug.show_layer_chunks_transparent,
                                "Show transparent chunks layer",
                            )
                            .changed();
                    });
                state.options_section_layers = layers_section.fully_open();

                let diagnostics_section = egui::CollapsingHeader::new("Diagnostics")
                    .default_open(state.options_section_diagnostics)
                    .show(ui, |ui| {
                        let mut selected_debug_mode = render_debug.cutout_debug_mode;
                        egui::ComboBox::from_label("Shader debug view")
                            .selected_text(match selected_debug_mode {
                                1 => "Pass id",
                                2 => "Atlas rgb",
                                3 => "Atlas alpha",
                                4 => "Vertex tint",
                                5 => "Linear depth",
                                6 => "Pass flags",
                                7 => "Alpha + pass",
                                8 => "Cutout lit flags",
                                _ => "Off",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut selected_debug_mode, 0, "Off");
                                ui.selectable_value(&mut selected_debug_mode, 1, "Pass id");
                                ui.selectable_value(&mut selected_debug_mode, 2, "Atlas rgb");
                                ui.selectable_value(&mut selected_debug_mode, 3, "Atlas alpha");
                                ui.selectable_value(&mut selected_debug_mode, 4, "Vertex tint");
                                ui.selectable_value(&mut selected_debug_mode, 5, "Linear depth");
                                ui.selectable_value(&mut selected_debug_mode, 6, "Pass flags");
                                ui.selectable_value(&mut selected_debug_mode, 7, "Alpha + pass");
                                ui.selectable_value(
                                    &mut selected_debug_mode,
                                    8,
                                    "Cutout lit flags",
                                );
                            });
                        if selected_debug_mode != render_debug.cutout_debug_mode {
                            render_debug.cutout_debug_mode = selected_debug_mode;
                            options_changed = true;
                        }
                    });
                state.options_section_diagnostics = diagnostics_section.fully_open();

                let system_section = egui::CollapsingHeader::new("System")
                    .default_open(state.options_section_system)
                    .show(ui, |ui| {
                        if ui.button("Reset All Settings To Default").clicked() {
                            *render_debug = RenderDebugSettings::default();
                            state.vsync_enabled = false;
                            if let Ok(mut window) = window_query.get_single_mut() {
                                window.present_mode = PresentMode::AutoNoVsync;
                            }
                            state.options_dirty = true;
                            options_changed = true;
                        }
                        ui.add_space(8.0);
                        if ui.button("Visual Settings...").clicked() {
                            state.visual_settings_open = !state.visual_settings_open;
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label("Options file");
                            ui.text_edit_singleline(&mut state.options_path);
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Load").clicked() {
                                let options_path = state.options_path.clone();
                                if let Ok(mut window) = window_query.get_single_mut() {
                                    match load_client_options(
                                        &options_path,
                                        &mut state,
                                        &mut render_debug,
                                        &mut window,
                                    ) {
                                        Ok(()) => {
                                            state.options_status =
                                                format!("Loaded {}", options_path);
                                        }
                                        Err(err) => state.options_status = err,
                                    }
                                } else {
                                    state.options_status =
                                        "Unable to load options: primary window unavailable"
                                            .to_string();
                                }
                            }
                            if ui.button("Save").clicked() {
                                match save_client_options(
                                    &state.options_path,
                                    &state,
                                    &render_debug,
                                ) {
                                    Ok(()) => {
                                        state.options_status =
                                            format!("Saved {}", state.options_path);
                                        state.options_dirty = false;
                                    }
                                    Err(err) => state.options_status = err,
                                }
                            }
                        });
                        if !state.options_status.is_empty() {
                            ui.label(&state.options_status);
                        }
                    });
                state.options_section_system = system_section.fully_open();

                if options_changed {
                    state.options_dirty = true;
                    match save_client_options(&state.options_path, &state, &render_debug) {
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
                        let _ = save_client_options(&state.options_path, &state, &render_debug);
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
                        let _ = save_client_options(&state.options_path, &state, &render_debug);
                        state.options_dirty = false;
                    }
                    state.visual_settings_open = false;
                    ui_state.paused = false;
                }
            });

        if state.visual_settings_open {
            egui::Window::new("Visual Settings")
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    let mut options_changed = false;
                    options_changed |= ui
                        .add(
                            egui::Slider::new(&mut render_debug.color_saturation, 0.5..=1.8)
                                .text("Saturation"),
                        )
                        .changed();
                    options_changed |= ui
                        .add(
                            egui::Slider::new(&mut render_debug.color_contrast, 0.6..=1.6)
                                .text("Contrast"),
                        )
                        .changed();
                    options_changed |= ui
                        .add(
                            egui::Slider::new(&mut render_debug.color_brightness, -0.2..=0.2)
                                .text("Brightness"),
                        )
                        .changed();
                    options_changed |= ui
                        .add(
                            egui::Slider::new(&mut render_debug.color_gamma, 0.6..=1.8)
                                .text("Gamma"),
                        )
                        .changed();
                    if ui.button("Reset Color Grading").clicked() {
                        render_debug.color_saturation = 1.08;
                        render_debug.color_contrast = 1.06;
                        render_debug.color_brightness = 0.0;
                        render_debug.color_gamma = 1.0;
                        options_changed = true;
                    }
                    if options_changed {
                        state.options_dirty = true;
                        match save_client_options(&state.options_path, &state, &render_debug) {
                            Ok(()) => {
                                state.options_status = format!("Saved {}", state.options_path);
                                state.options_dirty = false;
                            }
                            Err(err) => state.options_status = err,
                        }
                    }
                });
        }
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

#[derive(Resource)]
pub struct ConnectUiState {
    pub username: String,
    pub server_address: String,
    pub auth_mode: AuthMode,
    pub prism_accounts_path: String,
    pub auth_accounts: Vec<UiAuthAccount>,
    pub selected_auth_account: usize,
    pub auth_accounts_loaded: bool,
    pub connect_feedback: String,
    pub vsync_enabled: bool,
    pub options_loaded: bool,
    pub options_dirty: bool,
    pub options_path: String,
    pub options_status: String,
    pub visual_settings_open: bool,
    pub options_section_general: bool,
    pub options_section_lighting: bool,
    pub options_section_water: bool,
    pub options_section_layers: bool,
    pub options_section_diagnostics: bool,
    pub options_section_system: bool,
    pub debug_items_open: bool,
    pub debug_items_filter: String,
    pub debug_items: Vec<InventoryItemStack>,
}
impl Default for ConnectUiState {
    fn default() -> Self {
        Self {
            username: "RustyPlayer".to_string(),
            server_address: "localhost:25565".to_string(),
            auth_mode: AuthMode::Authenticated,
            prism_accounts_path: default_prism_accounts_path(),
            auth_accounts: Vec::new(),
            selected_auth_account: 0,
            auth_accounts_loaded: false,
            connect_feedback: String::new(),
            vsync_enabled: false,
            options_loaded: false,
            options_dirty: false,
            options_path: DEFAULT_OPTIONS_PATH.to_string(),
            options_status: String::new(),
            visual_settings_open: false,
            options_section_general: false,
            options_section_lighting: false,
            options_section_water: false,
            options_section_layers: false,
            options_section_diagnostics: false,
            options_section_system: false,
            debug_items_open: false,
            debug_items_filter: String::new(),
            debug_items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct ClientOptionsFile {
    pub fov_deg: f32,
    pub render_distance_chunks: i32,
    pub shadows_enabled: bool,
    pub shadow_distance_scale: f32,
    pub fxaa_enabled: bool,
    pub aa_mode: String,
    pub manual_frustum_cull: bool,
    pub vsync_enabled: bool,
    pub lighting_quality: String,
    pub shadow_quality: String,
    pub shader_quality_mode: u8,
    pub enable_pbr_terrain_lighting: bool,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub sun_strength: f32,
    pub ambient_strength: f32,
    pub ambient_brightness: f32,
    pub sun_illuminance: f32,
    pub fill_illuminance: f32,
    pub fog_density: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub water_absorption: f32,
    pub water_fresnel: f32,
    pub shadow_map_size: u32,
    pub shadow_cascades: u8,
    pub shadow_max_distance: f32,
    pub shadow_first_cascade_far_bound: f32,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    pub color_saturation: f32,
    pub color_contrast: f32,
    pub color_brightness: f32,
    pub color_gamma: f32,
    pub voxel_ao_enabled: bool,
    pub voxel_ao_strength: f32,
    pub voxel_ao_cutout: bool,
    pub water_reflections_enabled: bool,
    pub water_reflection_screen_space: bool,
    pub water_reflection_strength: f32,
    pub water_reflection_near_boost: f32,
    pub water_reflection_blue_tint: bool,
    pub water_reflection_tint_strength: f32,
    pub water_wave_strength: f32,
    pub water_wave_speed: f32,
    pub water_wave_detail_strength: f32,
    pub water_wave_detail_scale: f32,
    pub water_wave_detail_speed: f32,
    pub water_reflection_edge_fade: f32,
    pub water_reflection_overscan: f32,
    pub water_reflection_sky_fill: f32,
    pub water_ssr_steps: u8,
    pub water_ssr_thickness: f32,
    pub water_ssr_max_distance: f32,
    pub water_ssr_stride: f32,
    pub cutout_debug_mode: u8,
    pub show_layer_entities: bool,
    pub show_layer_chunks_opaque: bool,
    pub show_layer_chunks_cutout: bool,
    pub show_layer_chunks_transparent: bool,
}

impl Default for ClientOptionsFile {
    fn default() -> Self {
        let render = RenderDebugSettings::default();
        Self {
            fov_deg: render.fov_deg,
            render_distance_chunks: render.render_distance_chunks,
            shadows_enabled: render.shadows_enabled,
            shadow_distance_scale: render.shadow_distance_scale,
            fxaa_enabled: render.fxaa_enabled,
            aa_mode: render.aa_mode.as_options_value().to_string(),
            manual_frustum_cull: render.manual_frustum_cull,
            vsync_enabled: false,
            lighting_quality: render.lighting_quality.as_options_value().to_string(),
            shadow_quality: render.shadow_quality.as_options_value().to_string(),
            shader_quality_mode: render.shader_quality_mode,
            enable_pbr_terrain_lighting: render.enable_pbr_terrain_lighting,
            sun_azimuth_deg: render.sun_azimuth_deg,
            sun_elevation_deg: render.sun_elevation_deg,
            sun_strength: render.sun_strength,
            ambient_strength: render.ambient_strength,
            ambient_brightness: render.ambient_brightness,
            sun_illuminance: render.sun_illuminance,
            fill_illuminance: render.fill_illuminance,
            fog_density: render.fog_density,
            fog_start: render.fog_start,
            fog_end: render.fog_end,
            water_absorption: render.water_absorption,
            water_fresnel: render.water_fresnel,
            shadow_map_size: render.shadow_map_size,
            shadow_cascades: render.shadow_cascades,
            shadow_max_distance: render.shadow_max_distance,
            shadow_first_cascade_far_bound: render.shadow_first_cascade_far_bound,
            shadow_depth_bias: render.shadow_depth_bias,
            shadow_normal_bias: render.shadow_normal_bias,
            color_saturation: render.color_saturation,
            color_contrast: render.color_contrast,
            color_brightness: render.color_brightness,
            color_gamma: render.color_gamma,
            voxel_ao_enabled: render.voxel_ao_enabled,
            voxel_ao_strength: render.voxel_ao_strength,
            voxel_ao_cutout: render.voxel_ao_cutout,
            water_reflections_enabled: render.water_reflections_enabled,
            water_reflection_screen_space: render.water_reflection_screen_space,
            water_reflection_strength: render.water_reflection_strength,
            water_reflection_near_boost: render.water_reflection_near_boost,
            water_reflection_blue_tint: render.water_reflection_blue_tint,
            water_reflection_tint_strength: render.water_reflection_tint_strength,
            water_wave_strength: render.water_wave_strength,
            water_wave_speed: render.water_wave_speed,
            water_wave_detail_strength: render.water_wave_detail_strength,
            water_wave_detail_scale: render.water_wave_detail_scale,
            water_wave_detail_speed: render.water_wave_detail_speed,
            water_reflection_edge_fade: render.water_reflection_edge_fade,
            water_reflection_overscan: render.water_reflection_overscan,
            water_reflection_sky_fill: render.water_reflection_sky_fill,
            water_ssr_steps: render.water_ssr_steps,
            water_ssr_thickness: render.water_ssr_thickness,
            water_ssr_max_distance: render.water_ssr_max_distance,
            water_ssr_stride: render.water_ssr_stride,
            cutout_debug_mode: render.cutout_debug_mode,
            show_layer_entities: render.show_layer_entities,
            show_layer_chunks_opaque: render.show_layer_chunks_opaque,
            show_layer_chunks_cutout: render.show_layer_chunks_cutout,
            show_layer_chunks_transparent: render.show_layer_chunks_transparent,
        }
    }
}

fn options_to_file(state: &ConnectUiState, render: &RenderDebugSettings) -> ClientOptionsFile {
    ClientOptionsFile {
        fov_deg: render.fov_deg,
        render_distance_chunks: render.render_distance_chunks,
        shadows_enabled: render.shadows_enabled,
        shadow_distance_scale: render.shadow_distance_scale,
        fxaa_enabled: render.fxaa_enabled,
        aa_mode: render.aa_mode.as_options_value().to_string(),
        manual_frustum_cull: render.manual_frustum_cull,
        vsync_enabled: state.vsync_enabled,
        lighting_quality: render.lighting_quality.as_options_value().to_string(),
        shadow_quality: render.shadow_quality.as_options_value().to_string(),
        shader_quality_mode: render.shader_quality_mode,
        enable_pbr_terrain_lighting: render.enable_pbr_terrain_lighting,
        sun_azimuth_deg: render.sun_azimuth_deg,
        sun_elevation_deg: render.sun_elevation_deg,
        sun_strength: render.sun_strength,
        ambient_strength: render.ambient_strength,
        ambient_brightness: render.ambient_brightness,
        sun_illuminance: render.sun_illuminance,
        fill_illuminance: render.fill_illuminance,
        fog_density: render.fog_density,
        fog_start: render.fog_start,
        fog_end: render.fog_end,
        water_absorption: render.water_absorption,
        water_fresnel: render.water_fresnel,
        shadow_map_size: render.shadow_map_size,
        shadow_cascades: render.shadow_cascades,
        shadow_max_distance: render.shadow_max_distance,
        shadow_first_cascade_far_bound: render.shadow_first_cascade_far_bound,
        shadow_depth_bias: render.shadow_depth_bias,
        shadow_normal_bias: render.shadow_normal_bias,
        color_saturation: render.color_saturation,
        color_contrast: render.color_contrast,
        color_brightness: render.color_brightness,
        color_gamma: render.color_gamma,
        voxel_ao_enabled: render.voxel_ao_enabled,
        voxel_ao_strength: render.voxel_ao_strength,
        voxel_ao_cutout: render.voxel_ao_cutout,
        water_reflections_enabled: render.water_reflections_enabled,
        water_reflection_screen_space: render.water_reflection_screen_space,
        water_reflection_strength: render.water_reflection_strength,
        water_reflection_near_boost: render.water_reflection_near_boost,
        water_reflection_blue_tint: render.water_reflection_blue_tint,
        water_reflection_tint_strength: render.water_reflection_tint_strength,
        water_wave_strength: render.water_wave_strength,
        water_wave_speed: render.water_wave_speed,
        water_wave_detail_strength: render.water_wave_detail_strength,
        water_wave_detail_scale: render.water_wave_detail_scale,
        water_wave_detail_speed: render.water_wave_detail_speed,
        water_reflection_edge_fade: render.water_reflection_edge_fade,
        water_reflection_overscan: render.water_reflection_overscan,
        water_reflection_sky_fill: render.water_reflection_sky_fill,
        water_ssr_steps: render.water_ssr_steps,
        water_ssr_thickness: render.water_ssr_thickness,
        water_ssr_max_distance: render.water_ssr_max_distance,
        water_ssr_stride: render.water_ssr_stride,
        cutout_debug_mode: render.cutout_debug_mode,
        show_layer_entities: render.show_layer_entities,
        show_layer_chunks_opaque: render.show_layer_chunks_opaque,
        show_layer_chunks_cutout: render.show_layer_chunks_cutout,
        show_layer_chunks_transparent: render.show_layer_chunks_transparent,
    }
}

fn apply_options(
    options: &ClientOptionsFile,
    state: &mut ConnectUiState,
    render: &mut RenderDebugSettings,
    window: &mut Window,
) {
    render.fov_deg = options.fov_deg.clamp(60.0, 140.0);
    render.render_distance_chunks = options.render_distance_chunks.clamp(2, 32);
    if let Some(preset) = LightingQualityPreset::from_options_value(&options.lighting_quality) {
        render.lighting_quality = preset;
    }
    if let Some(preset) = ShadowQualityPreset::from_options_value(&options.shadow_quality) {
        render.shadow_quality = preset;
    }
    render.shader_quality_mode = options.shader_quality_mode.clamp(0, 3);
    if let Some(mode) = AntiAliasingMode::from_options_value(&options.aa_mode) {
        render.aa_mode = mode;
    } else {
        // Backward compatibility for older options files without aa_mode.
        render.aa_mode = if options.fxaa_enabled {
            AntiAliasingMode::Fxaa
        } else {
            AntiAliasingMode::Off
        };
    }
    // Explicit toggles in options file override preset defaults.
    render.shadows_enabled = options.shadows_enabled;
    render.shadow_distance_scale = options.shadow_distance_scale.clamp(0.25, 20.0);
    render.fxaa_enabled = matches!(
        render.aa_mode,
        AntiAliasingMode::Fxaa | AntiAliasingMode::Msaa4 | AntiAliasingMode::Msaa8
    );
    render.manual_frustum_cull = options.manual_frustum_cull;
    render.enable_pbr_terrain_lighting = options.enable_pbr_terrain_lighting;
    render.sun_azimuth_deg = options.sun_azimuth_deg.clamp(-360.0, 360.0);
    render.sun_elevation_deg = options.sun_elevation_deg.clamp(-89.0, 89.0);
    render.sun_strength = options.sun_strength.clamp(0.0, 2.0);
    render.ambient_strength = options.ambient_strength.clamp(0.0, 2.0);
    render.ambient_brightness = options.ambient_brightness.clamp(0.0, 2.0);
    render.sun_illuminance = options.sun_illuminance.clamp(0.0, 50_000.0);
    render.fill_illuminance = options.fill_illuminance.clamp(0.0, 10_000.0);
    render.fog_density = options.fog_density.clamp(0.0, 0.1);
    render.fog_start = options.fog_start.clamp(0.0, 1_000.0);
    render.fog_end = options.fog_end.clamp(0.0, 2_000.0);
    render.water_absorption = options.water_absorption.clamp(0.0, 1.0);
    render.water_fresnel = options.water_fresnel.clamp(0.0, 1.0);
    render.shadow_map_size = options.shadow_map_size.clamp(256, 4096);
    render.shadow_cascades = options.shadow_cascades.clamp(1, 4);
    render.shadow_max_distance = options.shadow_max_distance.clamp(4.0, 500.0);
    render.shadow_first_cascade_far_bound =
        options.shadow_first_cascade_far_bound.clamp(1.0, 300.0);
    render.shadow_depth_bias = options.shadow_depth_bias.clamp(0.0, 0.2);
    render.shadow_normal_bias = options.shadow_normal_bias.clamp(0.0, 2.0);
    render.color_saturation = options.color_saturation.clamp(0.0, 2.0);
    render.color_contrast = options.color_contrast.clamp(0.0, 2.0);
    render.color_brightness = options.color_brightness.clamp(-0.5, 0.5);
    render.color_gamma = options.color_gamma.clamp(0.2, 2.5);
    render.voxel_ao_enabled = options.voxel_ao_enabled;
    render.voxel_ao_strength = options.voxel_ao_strength.clamp(0.0, 1.0);
    render.voxel_ao_cutout = options.voxel_ao_cutout;
    render.water_reflections_enabled = options.water_reflections_enabled;
    render.water_reflection_screen_space = options.water_reflection_screen_space;
    render.water_reflection_strength = options.water_reflection_strength.clamp(0.0, 3.0);
    render.water_reflection_near_boost = options.water_reflection_near_boost.clamp(0.0, 1.0);
    render.water_reflection_blue_tint = options.water_reflection_blue_tint;
    render.water_reflection_tint_strength = options.water_reflection_tint_strength.clamp(0.0, 2.0);
    render.water_wave_strength = options.water_wave_strength.clamp(0.0, 1.2);
    render.water_wave_speed = options.water_wave_speed.clamp(0.0, 4.0);
    render.water_wave_detail_strength = options.water_wave_detail_strength.clamp(0.0, 1.0);
    render.water_wave_detail_scale = options.water_wave_detail_scale.clamp(1.0, 8.0);
    render.water_wave_detail_speed = options.water_wave_detail_speed.clamp(0.0, 4.0);
    render.water_reflection_edge_fade = options.water_reflection_edge_fade.clamp(0.01, 0.5);
    render.water_reflection_overscan = options.water_reflection_overscan.clamp(1.0, 3.0);
    render.water_reflection_sky_fill = options.water_reflection_sky_fill.clamp(0.0, 1.0);
    render.water_ssr_steps = options.water_ssr_steps.clamp(4, 64);
    render.water_ssr_thickness = options.water_ssr_thickness.clamp(0.02, 2.0);
    render.water_ssr_max_distance = options.water_ssr_max_distance.clamp(4.0, 400.0);
    render.water_ssr_stride = options.water_ssr_stride.clamp(0.2, 8.0);
    render.cutout_debug_mode = options.cutout_debug_mode.clamp(0, 8);
    render.show_layer_entities = options.show_layer_entities;
    render.show_layer_chunks_opaque = options.show_layer_chunks_opaque;
    render.show_layer_chunks_cutout = options.show_layer_chunks_cutout;
    render.show_layer_chunks_transparent = options.show_layer_chunks_transparent;
    state.vsync_enabled = options.vsync_enabled;
    window.present_mode = if state.vsync_enabled {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
}

fn load_client_options(
    path: &str,
    state: &mut ConnectUiState,
    render: &mut RenderDebugSettings,
    window: &mut Window,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    let content = match std::fs::read_to_string(&path_buf) {
        Ok(v) => v,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let defaults = ClientOptionsFile::default();
            apply_options(&defaults, state, render, window);
            return save_client_options(path, state, render);
        }
        Err(err) => return Err(format!("Failed to read options file {}: {}", path, err)),
    };
    let parsed = toml::from_str::<ClientOptionsFile>(&content)
        .map_err(|err| format!("Invalid TOML options {}: {}", path, err))?;
    apply_options(&parsed, state, render, window);
    Ok(())
}

fn save_client_options(
    path: &str,
    state: &ConnectUiState,
    render: &RenderDebugSettings,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    if let Some(parent) = path_buf.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create options directory {}: {}",
                parent.display(),
                err
            )
        })?;
    }
    let body = toml::to_string_pretty(&options_to_file(state, render))
        .map_err(|err| format!("Failed to encode options TOML: {}", err))?;
    std::fs::write(&path_buf, body)
        .map_err(|err| format!("Failed to write options file {}: {}", path, err))
}

fn short_uuid(uuid: &str) -> String {
    uuid.chars().take(8).collect::<String>()
}

#[derive(Debug, Clone)]
pub struct UiAuthAccount {
    pub username: String,
    pub uuid: String,
    pub active: bool,
}

fn default_prism_accounts_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("PrismLauncher")
        .join("accounts.json")
        .display()
        .to_string()
}

fn load_prism_accounts(prism_path: &str) -> Vec<UiAuthAccount> {
    let Ok(raw) = std::fs::read_to_string(prism_path) else {
        return Vec::new();
    };
    let Ok(root) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(accounts) = root.get("accounts").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for acc in accounts {
        if acc.get("type").and_then(Value::as_str) != Some("MSA") {
            continue;
        }
        let username = acc
            .pointer("/profile/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut uuid = acc
            .pointer("/profile/id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        uuid.retain(|c| c != '-');
        let active = acc.get("active").and_then(Value::as_bool).unwrap_or(false);

        if username.is_empty() || uuid.len() != 32 {
            continue;
        }
        out.push(UiAuthAccount {
            username,
            uuid,
            active,
        });
    }
    out
}

fn draw_hotbar_ui(
    ctx: &egui::Context,
    inventory_state: &InventoryState,
    player_status: &PlayerStatus,
    item_icons: &mut ItemIconCache,
) {
    let is_creative = player_status.gamemode == 1;
    let armor_frac = equipped_armor_points(inventory_state) as f32 / 20.0;
    let health_frac = (player_status.health / 20.0).clamp(0.0, 1.0);
    let hunger_frac = (player_status.food as f32 / 20.0).clamp(0.0, 1.0);
    let xp_frac = player_status.experience_bar.clamp(0.0, 1.0);
    let hotbar_width = INVENTORY_SLOT_SIZE * 9.0 + INVENTORY_SLOT_SPACING * 8.0;

    egui::Area::new(egui::Id::new("hotbar_overlay"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0, -12.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_black_alpha(170))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(64)))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    if !is_creative {
                        let (armor_rect, _) = ui.allocate_exact_size(
                            egui::vec2(hotbar_width, 7.0),
                            egui::Sense::hover(),
                        );
                        draw_stat_bar(
                            ui.painter(),
                            armor_rect,
                            armor_frac.clamp(0.0, 1.0),
                            egui::Color32::from_rgb(126, 170, 218),
                        );
                        ui.add_space(3.0);
                        let (bars_rect, _) = ui.allocate_exact_size(
                            egui::vec2(hotbar_width, 10.0),
                            egui::Sense::hover(),
                        );
                        let half_width = (hotbar_width - INVENTORY_SLOT_SPACING) * 0.5;
                        let health_rect = egui::Rect::from_min_size(
                            bars_rect.min,
                            egui::vec2(half_width, bars_rect.height()),
                        );
                        let hunger_rect = egui::Rect::from_min_size(
                            egui::pos2(health_rect.max.x + INVENTORY_SLOT_SPACING, bars_rect.min.y),
                            egui::vec2(half_width, bars_rect.height()),
                        );
                        draw_stat_bar(
                            ui.painter(),
                            health_rect,
                            health_frac,
                            egui::Color32::from_rgb(170, 46, 46),
                        );
                        draw_stat_bar(
                            ui.painter(),
                            hunger_rect,
                            hunger_frac,
                            egui::Color32::from_rgb(181, 122, 43),
                        );
                        ui.add_space(3.0);
                    }
                    let (xp_rect, _) =
                        ui.allocate_exact_size(egui::vec2(hotbar_width, 7.0), egui::Sense::hover());
                    draw_stat_bar(
                        ui.painter(),
                        xp_rect,
                        xp_frac,
                        egui::Color32::from_rgb(110, 196, 64),
                    );
                    ui.add_space(4.0);

                    egui::Grid::new("hud_hotbar_grid")
                        .spacing(egui::Vec2::new(
                            INVENTORY_SLOT_SPACING,
                            INVENTORY_SLOT_SPACING,
                        ))
                        .show(ui, |ui| {
                            for hotbar_idx in 0..9u8 {
                                let item = inventory_state.hotbar_item(hotbar_idx);
                                let selected = inventory_state.selected_hotbar_slot == hotbar_idx;
                                let _ = draw_slot(
                                    ctx,
                                    item_icons,
                                    ui,
                                    item.as_ref(),
                                    selected,
                                    INVENTORY_SLOT_SIZE,
                                    false,
                                );
                            }
                            ui.end_row();
                        });
                });
        });
}

fn draw_stat_bar(painter: &egui::Painter, rect: egui::Rect, progress: f32, fill: egui::Color32) {
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(92));
    painter.rect(
        rect,
        2.0,
        egui::Color32::from_gray(28),
        stroke,
        egui::StrokeKind::Outside,
    );
    let width = (rect.width() - 2.0) * progress.clamp(0.0, 1.0);
    if width <= 0.0 {
        return;
    }
    let fill_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(1.0, 1.0),
        egui::vec2(width, rect.height() - 2.0),
    );
    painter.rect_filled(fill_rect, 1.5, fill);
}

fn build_debug_item_list() -> Vec<InventoryItemStack> {
    let mut out = Vec::new();
    for block_id in 1u16..=197u16 {
        if block_registry_key(block_id).is_none() {
            continue;
        }
        for damage in debug_block_meta_variants(block_id) {
            out.push(InventoryItemStack {
                item_id: block_id as i32,
                count: 1,
                damage,
                meta: Default::default(),
            });
        }
    }
    for item_id in 0i32..=5000i32 {
        if item_registry_key(item_id).is_none() {
            continue;
        }
        out.push(InventoryItemStack {
            item_id,
            count: 1,
            damage: 0,
            meta: Default::default(),
        });
    }
    out
}

fn debug_block_meta_variants(block_id: u16) -> Vec<i16> {
    let metas: &[i16] = match block_id {
        1 | 3 | 6 | 12 | 17 | 18 | 24 | 35 | 38 | 43 | 44 | 95 | 97 | 98 | 126 | 159 | 160
        | 161 | 162 | 171 | 175 => &[0, 1, 2, 3, 4, 5, 6, 7],
        _ => &[0],
    };
    let mut out = Vec::new();
    for &m in metas {
        out.push(m);
    }
    if matches!(block_id, 35 | 95 | 159 | 160 | 171) {
        out.clear();
        for m in 0..=15i16 {
            out.push(m);
        }
    }
    out
}

fn draw_debug_item_browser(
    ctx: &egui::Context,
    state: &mut ConnectUiState,
    item_icons: &mut ItemIconCache,
    to_net: &ToNet,
) {
    let give_target = debug_give_target(state);
    egui::Window::new("Debug Item Browser")
        .open(&mut state.debug_items_open)
        .resizable(true)
        .default_width(900.0)
        .default_height(620.0)
        .show(ctx, |ui| {
            if state.debug_items.is_empty() {
                state.debug_items = build_debug_item_list();
            }

            ui.horizontal(|ui| {
                ui.label("Filter");
                ui.text_edit_singleline(&mut state.debug_items_filter);
                if ui.button("Clear").clicked() {
                    state.debug_items_filter.clear();
                }
                ui.separator();
                ui.label(format!("Items: {}", state.debug_items.len()));
                ui.separator();
                ui.label("Click item: send /give");
            });
            ui.add_space(6.0);

            let filter = state.debug_items_filter.trim().to_ascii_lowercase();
            let filtered: Vec<&InventoryItemStack> = state
                .debug_items
                .iter()
                .filter(|stack| {
                    if filter.is_empty() {
                        return true;
                    }
                    let name = item_name(stack.item_id).to_ascii_lowercase();
                    let key = item_registry_key(stack.item_id)
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let id_text = stack.item_id.to_string();
                    name.contains(&filter) || key.contains(&filter) || id_text.contains(&filter)
                })
                .collect();

            let columns = ((ui.available_width() / DEBUG_ITEM_CELL).floor() as usize).max(1);
            let mut hovered: Option<InventoryItemStack> = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut i = 0usize;
                while i < filtered.len() {
                    ui.horizontal(|ui| {
                        for _ in 0..columns {
                            if i >= filtered.len() {
                                break;
                            }
                            let stack = filtered[i];
                            let response = draw_slot(
                                ctx,
                                item_icons,
                                ui,
                                Some(stack),
                                false,
                                DEBUG_ITEM_CELL - 8.0,
                                false,
                            );
                            if response.hovered() {
                                hovered = Some(stack.clone());
                            }
                            if response.clicked() {
                                let cmd = debug_give_command(stack, &give_target);
                                warn!(
                                    "debug give click item_id={} damage={} cmd={}",
                                    stack.item_id, stack.damage, cmd
                                );
                                if let Err(err) = to_net.0.send(ToNetMessage::ChatMessage(cmd)) {
                                    warn!("debug give send failed: {}", err);
                                } else {
                                    warn!("debug give send ok");
                                }
                            }
                            i += 1;
                        }
                    });
                    ui.add_space(2.0);
                }
            });

            if let Some(stack) = hovered.as_ref() {
                draw_inventory_item_tooltip(ctx, stack);
            }
        });
}

fn debug_give_command(stack: &InventoryItemStack, target: &str) -> String {
    let item = item_registry_key(stack.item_id)
        .map(str::to_string)
        .or_else(|| {
            u16::try_from(stack.item_id)
                .ok()
                .and_then(block_registry_key)
                .map(str::to_string)
        })
        .unwrap_or_else(|| stack.item_id.to_string());
    let damage = i32::from(stack.damage.max(0));
    format!("/give {target} {item} 1 {damage}")
}

fn debug_give_target(state: &ConnectUiState) -> String {
    if matches!(state.auth_mode, AuthMode::Authenticated)
        && !state.auth_accounts.is_empty()
        && state.selected_auth_account < state.auth_accounts.len()
    {
        let chosen = state.auth_accounts[state.selected_auth_account]
            .username
            .trim()
            .to_string();
        if !chosen.is_empty() {
            return chosen;
        }
    }
    let username = state.username.trim();
    if username.is_empty() {
        "@p".to_string()
    } else {
        username.to_string()
    }
}

fn equipped_armor_points(inventory_state: &InventoryState) -> i32 {
    let mut points = 0;
    for slot in [5usize, 6usize, 7usize, 8usize] {
        if let Some(Some(stack)) = inventory_state.player_slots.get(slot) {
            points += armor_points_for_item(stack.item_id);
        }
    }
    points.clamp(0, 20)
}

fn armor_points_for_item(item_id: i32) -> i32 {
    match item_id {
        298 => 1, // leather helmet
        299 => 3, // leather chestplate
        300 => 2, // leather leggings
        301 => 1, // leather boots
        302 => 1, // chain helmet
        303 => 5, // chain chestplate
        304 => 4, // chain leggings
        305 => 1, // chain boots
        306 => 2, // iron helmet
        307 => 6, // iron chestplate
        308 => 5, // iron leggings
        309 => 2, // iron boots
        310 => 3, // diamond helmet
        311 => 8, // diamond chestplate
        312 => 6, // diamond leggings
        313 => 3, // diamond boots
        314 => 2, // gold helmet
        315 => 5, // gold chestplate
        316 => 3, // gold leggings
        317 => 1, // gold boots
        _ => 0,
    }
}

fn draw_inventory_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    inventory_state: &mut InventoryState,
    item_icons: &mut ItemIconCache,
) {
    if let Some(window) = inventory_state
        .open_window
        .clone()
        .filter(|window| window.id != 0)
    {
        draw_container_inventory_grid(ctx, ui, to_net, keys, inventory_state, item_icons, &window);
        return;
    }

    draw_player_inventory_grid(ctx, ui, to_net, keys, inventory_state, item_icons);
}

fn draw_player_inventory_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    inventory_state: &mut InventoryState,
    item_icons: &mut ItemIconCache,
) {
    ui.label("Survival Inventory");
    ui.add_space(4.0);
    let mut hovered_any_slot = false;
    let mut hovered_item: Option<InventoryItemStack> = None;

    ui.label("Armor");
    ui.add_space(4.0);
    egui::Grid::new("inventory_armor_row")
        .spacing(egui::Vec2::new(
            INVENTORY_SLOT_SPACING,
            INVENTORY_SLOT_SPACING,
        ))
        .show(ui, |ui| {
            for slot in [5usize, 6usize, 7usize, 8usize] {
                let item = inventory_state.player_slots.get(slot).cloned().flatten();
                let response = draw_slot(
                    ctx,
                    item_icons,
                    ui,
                    item.as_ref(),
                    false,
                    INVENTORY_SLOT_SIZE,
                    true,
                );
                if response.hovered() {
                    hovered_any_slot = true;
                    hovered_item = item;
                }
                handle_inventory_slot_interaction(
                    response,
                    0,
                    0,
                    slot as i16,
                    keys,
                    to_net,
                    inventory_state,
                );
            }
            ui.end_row();
        });

    ui.add_space(8.0);
    egui::Grid::new("inventory_main_grid")
        .spacing(egui::Vec2::new(
            INVENTORY_SLOT_SPACING,
            INVENTORY_SLOT_SPACING,
        ))
        .show(ui, |ui| {
            for row in 0..3usize {
                for col in 0..9usize {
                    let slot = 9 + row * 9 + col;
                    let item = inventory_state.player_slots.get(slot).cloned().flatten();
                    let response = draw_slot(
                        ctx,
                        item_icons,
                        ui,
                        item.as_ref(),
                        false,
                        INVENTORY_SLOT_SIZE,
                        true,
                    );
                    if response.hovered() {
                        hovered_any_slot = true;
                        hovered_item = item;
                    }
                    handle_inventory_slot_interaction(
                        response,
                        0,
                        0,
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
                let response = draw_slot(
                    ctx,
                    item_icons,
                    ui,
                    item.as_ref(),
                    selected,
                    INVENTORY_SLOT_SIZE,
                    true,
                );
                if response.hovered() {
                    hovered_any_slot = true;
                    hovered_item = item;
                }
                handle_inventory_slot_interaction(
                    response,
                    0,
                    0,
                    slot,
                    keys,
                    to_net,
                    inventory_state,
                );
            }
            ui.end_row();
        });

    let clicked_primary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
    let clicked_secondary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    if !hovered_any_slot && ui.rect_contains_pointer(ui.max_rect()) {
        if clicked_primary {
            send_inventory_click(0, 0, -999, 0, 0, to_net, inventory_state);
        } else if clicked_secondary {
            send_inventory_click(0, 0, -999, 1, 0, to_net, inventory_state);
        }
    }

    if let Some(stack) = hovered_item.as_ref() {
        draw_inventory_item_tooltip(ctx, stack);
    }
}

fn draw_container_inventory_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    inventory_state: &mut InventoryState,
    item_icons: &mut ItemIconCache,
    window: &InventoryWindowInfo,
) {
    let unique_slots = container_unique_slot_count(inventory_state, window);
    let cols = container_layout_columns(&window.kind, unique_slots);
    let rows = if unique_slots == 0 {
        0
    } else {
        unique_slots.div_ceil(cols)
    };

    ui.label(format!("{} ({})", window.title, window.kind));
    ui.add_space(4.0);
    let mut hovered_any_slot = false;
    let mut hovered_item: Option<InventoryItemStack> = None;

    if rows > 0 {
        egui::Grid::new(format!("container_grid_{}", window.id))
            .spacing(egui::Vec2::new(
                INVENTORY_SLOT_SPACING,
                INVENTORY_SLOT_SPACING,
            ))
            .show(ui, |ui| {
                for row in 0..rows {
                    for col in 0..cols {
                        let slot = row * cols + col;
                        if slot >= unique_slots {
                            ui.allocate_exact_size(
                                egui::Vec2::splat(INVENTORY_SLOT_SIZE),
                                egui::Sense::hover(),
                            );
                            continue;
                        }
                        let item = inventory_state
                            .window_slots
                            .get(&window.id)
                            .and_then(|slots| slots.get(slot))
                            .cloned()
                            .flatten();
                        let response = draw_slot(
                            ctx,
                            item_icons,
                            ui,
                            item.as_ref(),
                            false,
                            INVENTORY_SLOT_SIZE,
                            true,
                        );
                        if response.hovered() {
                            hovered_any_slot = true;
                            hovered_item = item;
                        }
                        handle_inventory_slot_interaction(
                            response,
                            window.id,
                            unique_slots,
                            slot as i16,
                            keys,
                            to_net,
                            inventory_state,
                        );
                    }
                    ui.end_row();
                }
            });
    }

    ui.add_space(8.0);
    ui.label("Inventory");
    ui.add_space(4.0);
    egui::Grid::new(format!("container_player_main_grid_{}", window.id))
        .spacing(egui::Vec2::new(
            INVENTORY_SLOT_SPACING,
            INVENTORY_SLOT_SPACING,
        ))
        .show(ui, |ui| {
            for row in 0..3usize {
                for col in 0..9usize {
                    let player_offset = row * 9 + col;
                    let slot = unique_slots + player_offset;
                    let item = container_player_slot_item(
                        inventory_state,
                        window.id,
                        unique_slots,
                        player_offset,
                    );
                    let response = draw_slot(
                        ctx,
                        item_icons,
                        ui,
                        item.as_ref(),
                        false,
                        INVENTORY_SLOT_SIZE,
                        true,
                    );
                    if response.hovered() {
                        hovered_any_slot = true;
                        hovered_item = item;
                    }
                    handle_inventory_slot_interaction(
                        response,
                        window.id,
                        unique_slots,
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
    egui::Grid::new(format!("container_player_hotbar_grid_{}", window.id))
        .spacing(egui::Vec2::new(
            INVENTORY_SLOT_SPACING,
            INVENTORY_SLOT_SPACING,
        ))
        .show(ui, |ui| {
            for hotbar_idx in 0..9usize {
                let player_offset = 27 + hotbar_idx;
                let slot = unique_slots + player_offset;
                let item = container_player_slot_item(
                    inventory_state,
                    window.id,
                    unique_slots,
                    player_offset,
                );
                let selected = inventory_state.selected_hotbar_slot as usize == hotbar_idx;
                let response = draw_slot(
                    ctx,
                    item_icons,
                    ui,
                    item.as_ref(),
                    selected,
                    INVENTORY_SLOT_SIZE,
                    true,
                );
                if response.hovered() {
                    hovered_any_slot = true;
                    hovered_item = item;
                }
                handle_inventory_slot_interaction(
                    response,
                    window.id,
                    unique_slots,
                    slot as i16,
                    keys,
                    to_net,
                    inventory_state,
                );
            }
            ui.end_row();
        });

    let clicked_primary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary));
    let clicked_secondary = ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
    if !hovered_any_slot && ui.rect_contains_pointer(ui.max_rect()) {
        if clicked_primary {
            send_inventory_click(window.id, unique_slots, -999, 0, 0, to_net, inventory_state);
        } else if clicked_secondary {
            send_inventory_click(window.id, unique_slots, -999, 1, 0, to_net, inventory_state);
        }
    }

    if let Some(stack) = hovered_item.as_ref() {
        draw_inventory_item_tooltip(ctx, stack);
    }
}

fn container_unique_slot_count(
    inventory_state: &InventoryState,
    window: &InventoryWindowInfo,
) -> usize {
    let declared = window.slot_count as usize;
    let actual_len = inventory_state
        .window_slots
        .get(&window.id)
        .map_or(0, std::vec::Vec::len);

    if declared > 0 {
        declared
    } else {
        actual_len.saturating_sub(36)
    }
}

fn container_layout_columns(kind: &str, unique_slots: usize) -> usize {
    let normalized = kind.to_ascii_lowercase();
    if normalized.contains("furnace") || normalized.contains("anvil") {
        return 3;
    }
    if normalized.contains("hopper") {
        return 5;
    }
    if normalized.contains("beacon") {
        return 1;
    }
    if normalized.contains("brewing") {
        return 4;
    }
    if normalized.contains("enchant") {
        return 2;
    }
    if normalized.contains("dispenser")
        || normalized.contains("dropper")
        || normalized.contains("chest")
        || normalized.contains("container")
        || normalized == "type_0"
        || normalized == "type_3"
        || normalized == "type_10"
    {
        return 9;
    }
    if normalized == "type_2" || normalized == "type_8" {
        return 3;
    }
    if normalized == "type_9" {
        return 5;
    }
    if unique_slots.is_multiple_of(9) {
        9
    } else if unique_slots.is_multiple_of(5) {
        5
    } else if unique_slots.is_multiple_of(3) {
        3
    } else {
        unique_slots.clamp(1, 9)
    }
}

fn container_player_slot_item(
    inventory_state: &InventoryState,
    window_id: u8,
    unique_slots: usize,
    player_offset: usize,
) -> Option<InventoryItemStack> {
    let window_idx = unique_slots + player_offset;
    if let Some(item) = inventory_state
        .window_slots
        .get(&window_id)
        .and_then(|slots| slots.get(window_idx))
        .cloned()
        .flatten()
    {
        return Some(item);
    }

    let player_slot = if player_offset < 27 {
        9 + player_offset
    } else {
        36 + (player_offset - 27)
    };
    inventory_state
        .player_slots
        .get(player_slot)
        .cloned()
        .flatten()
}

fn draw_slot(
    ctx: &egui::Context,
    item_icons: &mut ItemIconCache,
    ui: &mut egui::Ui,
    item: Option<&InventoryItemStack>,
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
        let mut icon_drawn = false;
        if let Some(texture_id) = item_icons.texture_for_stack(ctx, stack) {
            let icon_rect = rect.shrink(4.0);
            ui.painter().image(
                texture_id,
                icon_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            icon_drawn = true;
        }
        if !icon_drawn {
            let label = item_short_label(stack.item_id);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(11.0),
                egui::Color32::WHITE,
            );
        }
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

fn draw_inventory_item_tooltip(ctx: &egui::Context, stack: &InventoryItemStack) {
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    egui::Area::new(egui::Id::new("inventory_item_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos + egui::vec2(14.0, 14.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                draw_item_tooltip(ui, stack);
            });
        });
}

fn draw_chat_message(ui: &mut egui::Ui, msg: &str) {
    let segments = parse_legacy_chat_segments(msg);
    ui.horizontal_wrapped(|ui| {
        for segment in segments {
            let mut rich = egui::RichText::new(segment.text).color(segment.color);
            if segment.bold {
                rich = rich.strong();
            }
            if segment.italic {
                rich = rich.italics();
            }
            if segment.underlined {
                rich = rich.underline();
            }
            if segment.strikethrough {
                rich = rich.strikethrough();
            }
            ui.label(rich);
        }
    });
}

#[derive(Clone)]
struct ChatSegment {
    text: String,
    color: egui::Color32,
    bold: bool,
    italic: bool,
    underlined: bool,
    strikethrough: bool,
}

fn parse_legacy_chat_segments(msg: &str) -> Vec<ChatSegment> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut color = egui::Color32::from_rgb(230, 230, 230);
    let mut bold = false;
    let mut italic = false;
    let mut underlined = false;
    let mut strikethrough = false;

    let mut chars = msg.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '§' {
            buf.push(ch);
            continue;
        }
        let Some(code) = chars.next() else {
            buf.push(ch);
            break;
        };
        if !buf.is_empty() {
            out.push(ChatSegment {
                text: std::mem::take(&mut buf),
                color,
                bold,
                italic,
                underlined,
                strikethrough,
            });
        }
        match code.to_ascii_lowercase() {
            '0' => color = egui::Color32::from_rgb(0, 0, 0),
            '1' => color = egui::Color32::from_rgb(0, 0, 170),
            '2' => color = egui::Color32::from_rgb(0, 170, 0),
            '3' => color = egui::Color32::from_rgb(0, 170, 170),
            '4' => color = egui::Color32::from_rgb(170, 0, 0),
            '5' => color = egui::Color32::from_rgb(170, 0, 170),
            '6' => color = egui::Color32::from_rgb(255, 170, 0),
            '7' => color = egui::Color32::from_rgb(170, 170, 170),
            '8' => color = egui::Color32::from_rgb(85, 85, 85),
            '9' => color = egui::Color32::from_rgb(85, 85, 255),
            'a' => color = egui::Color32::from_rgb(85, 255, 85),
            'b' => color = egui::Color32::from_rgb(85, 255, 255),
            'c' => color = egui::Color32::from_rgb(255, 85, 85),
            'd' => color = egui::Color32::from_rgb(255, 85, 255),
            'e' => color = egui::Color32::from_rgb(255, 255, 85),
            'f' => color = egui::Color32::from_rgb(255, 255, 255),
            'k' => {}
            'l' => bold = true,
            'm' => strikethrough = true,
            'n' => underlined = true,
            'o' => italic = true,
            'r' => {
                color = egui::Color32::from_rgb(230, 230, 230);
                bold = false;
                italic = false;
                underlined = false;
                strikethrough = false;
            }
            _ => {}
        }
    }

    if !buf.is_empty() || out.is_empty() {
        out.push(ChatSegment {
            text: buf,
            color,
            bold,
            italic,
            underlined,
            strikethrough,
        });
    }
    out
}

fn draw_item_tooltip(ui: &mut egui::Ui, stack: &InventoryItemStack) {
    let display_name = stack
        .meta
        .display_name
        .as_deref()
        .unwrap_or_else(|| item_name(stack.item_id));
    ui.label(egui::RichText::new(display_name).strong());
    ui.label(egui::RichText::new(format!("Count: {}", stack.count)).small());
    ui.label(egui::RichText::new(format!("ID: {}  Meta: {}", stack.item_id, stack.damage)).small());

    if let Some(max) = item_max_durability(stack.item_id) {
        let remaining = (max as i32 - stack.damage.max(0) as i32).max(0);
        ui.label(egui::RichText::new(format!("Durability: {remaining}/{max}")).small());
    }

    if stack.meta.unbreakable {
        ui.label(egui::RichText::new("Unbreakable").small());
    }

    if let Some(repair_cost) = stack.meta.repair_cost {
        ui.label(egui::RichText::new(format!("Repair Cost: {repair_cost}")).small());
    }

    for ench in &stack.meta.enchantments {
        let ench_name = enchantment_name(ench.id);
        ui.label(
            egui::RichText::new(format!(
                "{ench_name} {}",
                format_enchantment_level(ench.level)
            ))
            .small()
            .color(egui::Color32::from_rgb(120, 80, 220)),
        );
    }

    for lore_line in &stack.meta.lore {
        ui.label(
            egui::RichText::new(lore_line.as_str())
                .small()
                .italics()
                .color(egui::Color32::from_gray(180)),
        );
    }
}

fn enchantment_name(id: i16) -> &'static str {
    match id {
        0 => "Protection",
        1 => "Fire Protection",
        2 => "Feather Falling",
        3 => "Blast Protection",
        4 => "Projectile Protection",
        5 => "Respiration",
        6 => "Aqua Affinity",
        7 => "Thorns",
        8 => "Depth Strider",
        16 => "Sharpness",
        17 => "Smite",
        18 => "Bane of Arthropods",
        19 => "Knockback",
        20 => "Fire Aspect",
        21 => "Looting",
        32 => "Efficiency",
        33 => "Silk Touch",
        34 => "Unbreaking",
        35 => "Fortune",
        48 => "Power",
        49 => "Punch",
        50 => "Flame",
        51 => "Infinity",
        61 => "Luck of the Sea",
        62 => "Lure",
        _ => "Enchantment",
    }
}

fn format_enchantment_level(level: i16) -> String {
    match level {
        1 => "I".to_string(),
        2 => "II".to_string(),
        3 => "III".to_string(),
        4 => "IV".to_string(),
        5 => "V".to_string(),
        6 => "VI".to_string(),
        7 => "VII".to_string(),
        8 => "VIII".to_string(),
        9 => "IX".to_string(),
        10 => "X".to_string(),
        _ => level.to_string(),
    }
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
        _ => item_name(item_id),
    }
}

fn draw_inventory_cursor_item(
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

fn handle_inventory_slot_interaction(
    response: egui::Response,
    window_id: u8,
    window_unique_slots: usize,
    slot: i16,
    keys: &ButtonInput<KeyCode>,
    to_net: &ToNet,
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

fn send_inventory_click(
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

fn close_open_window_if_needed(to_net: &ToNet, inventory_state: &mut InventoryState) {
    if let Some(window) = inventory_state.open_window.take() {
        if window.id != 0 {
            let _ = to_net.0.send(ToNetMessage::CloseWindow { id: window.id });
        }
    }
}

#[derive(Resource)]
struct ItemIconCache {
    loaded: HashMap<(i32, i16), egui::TextureHandle>,
    missing: HashSet<(i32, i16)>,
    block_model_resolver: BlockModelResolver,
    block_texture_images: HashMap<String, Option<egui::ColorImage>>,
    logged_stone_fallback: HashSet<(i32, i16)>,
    logged_model_fallback: HashSet<(i32, i16)>,
    logged_resolution_path: HashSet<(i32, i16)>,
}

impl Default for ItemIconCache {
    fn default() -> Self {
        Self {
            loaded: HashMap::new(),
            missing: HashSet::new(),
            block_model_resolver: BlockModelResolver::new(default_model_roots()),
            block_texture_images: HashMap::new(),
            logged_stone_fallback: HashSet::new(),
            logged_model_fallback: HashSet::new(),
            logged_resolution_path: HashSet::new(),
        }
    }
}

impl ItemIconCache {
    fn texture_for_stack(
        &mut self,
        ctx: &egui::Context,
        stack: &InventoryItemStack,
    ) -> Option<egui::TextureId> {
        let key = (stack.item_id, stack.damage);
        if let Some(handle) = self.loaded.get(&key) {
            return Some(handle.id());
        }
        if stack.damage != 0 {
            if let Some(handle) = self.loaded.get(&(stack.item_id, 0)) {
                return Some(handle.id());
            }
        }
        if self.missing.contains(&key) {
            return None;
        }

        if let Some(image) = generate_isometric_block_icon(
            stack.item_id,
            stack.damage,
            &mut self.block_model_resolver,
            &mut self.block_texture_images,
            &mut self.logged_stone_fallback,
            &mut self.logged_model_fallback,
            &mut self.logged_resolution_path,
        ) {
            let texture_name = format!("item_icon_iso_{}_{}", stack.item_id, stack.damage);
            let handle = ctx.load_texture(texture_name, image, egui::TextureOptions::NEAREST);
            let id = handle.id();
            self.loaded.insert(key, handle);
            return Some(id);
        }

        let candidates = item_texture_candidates(stack.item_id, stack.damage);
        for rel_path in candidates {
            let full_path = texturepack_textures_root().join(&rel_path);
            if !full_path.exists() {
                continue;
            }
            let Some(color_image) = load_color_image(&full_path) else {
                continue;
            };
            let texture_name = format!("item_icon_{}_{}_{}", stack.item_id, stack.damage, rel_path);
            let handle = ctx.load_texture(texture_name, color_image, egui::TextureOptions::NEAREST);
            let id = handle.id();
            self.loaded.insert(key, handle);
            return Some(id);
        }

        if stack.damage != 0 {
            let fallback_key = (stack.item_id, 0);
            if let Some(handle) = self.loaded.get(&fallback_key) {
                return Some(handle.id());
            }
        }

        self.missing.insert(key);
        None
    }
}

fn generate_isometric_block_icon(
    item_id: i32,
    damage: i16,
    resolver: &mut BlockModelResolver,
    texture_cache: &mut HashMap<String, Option<egui::ColorImage>>,
    logged_stone_fallback: &mut HashSet<(i32, i16)>,
    logged_model_fallback: &mut HashSet<(i32, i16)>,
    logged_resolution_path: &mut HashSet<(i32, i16)>,
) -> Option<egui::ColorImage> {
    let block_id = u16::try_from(item_id).ok()?;
    if block_registry_key(block_id).is_none() {
        return None;
    }

    if let Some(mut quads) = resolver.icon_quads_for_meta(block_id, damage as u8) {
        quads.sort_by(|a, b| {
            quad_depth(a)
                .partial_cmp(&quad_depth(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
        let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
        let mut rendered_any = false;
        for quad in quads {
            let Some(tex) = load_model_texture(&quad.texture_path, texture_cache) else {
                continue;
            };
            rendered_any = true;
            let tint = quad
                .tint_index
                .and_then(|_| icon_tint_color(block_id, damage))
                .unwrap_or([255, 255, 255]);
            raster_iso_quad(&mut out, &mut depth, &quad, &tex, tint);
        }
        if rendered_any {
            if logged_resolution_path.insert((item_id, damage)) {
                warn!(
                    "[isometric-debug] path=id:{} meta:{} source:blockstate-model key={:?}",
                    item_id,
                    damage,
                    block_registry_key(block_id)
                );
            }
            return Some(out);
        }
        if logged_model_fallback.insert((item_id, damage)) {
            warn!(
                "[isometric-debug] model texture fallback id={} meta={} key={:?}",
                item_id,
                damage,
                block_registry_key(block_id)
            );
        }
    }

    if let Some(quads) = resolver.block_item_icon_quads(block_id, damage as u8) {
        let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
        let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
        let mut rendered_any = false;
        for quad in quads {
            let Some(tex) = load_model_texture(&quad.texture_path, texture_cache) else {
                continue;
            };
            rendered_any = true;
            let tint = quad
                .tint_index
                .and_then(|_| icon_tint_color(block_id, damage))
                .unwrap_or([255, 255, 255]);
            raster_iso_quad(&mut out, &mut depth, &quad, &tex, tint);
        }
        if rendered_any {
            if logged_resolution_path.insert((item_id, damage)) {
                warn!(
                    "[isometric-debug] path=id:{} meta:{} source:item-model key={:?}",
                    item_id,
                    damage,
                    block_registry_key(block_id)
                );
            }
            return Some(out);
        }
    }

    // Guaranteed fallback: render a textured isometric cube so block items are never flat.
    let top_name = resolver
        .face_texture_name_for_meta(block_id, damage as u8, ModelFace::PosY)
        .or_else(|| fallback_block_face_texture(block_id, damage, BlockFace::Up))
        .unwrap_or_else(|| block_texture_name(block_id, BlockFace::Up).to_string());
    let east_name = resolver
        .face_texture_name_for_meta(block_id, damage as u8, ModelFace::PosX)
        .or_else(|| fallback_block_face_texture(block_id, damage, BlockFace::East))
        .unwrap_or_else(|| block_texture_name(block_id, BlockFace::East).to_string());
    let south_name = resolver
        .face_texture_name_for_meta(block_id, damage as u8, ModelFace::PosZ)
        .or_else(|| fallback_block_face_texture(block_id, damage, BlockFace::South))
        .unwrap_or_else(|| block_texture_name(block_id, BlockFace::South).to_string());
    let top = load_block_texture(&top_name, texture_cache)?;
    let east = load_block_texture(&east_name, texture_cache)?;
    let south = load_block_texture(&south_name, texture_cache)?;
    if block_id != 1 && (top_name == "stone.png" || east_name == "stone.png" || south_name == "stone.png")
    {
        if logged_stone_fallback.insert((item_id, damage)) {
            warn!(
                "[isometric-debug] stone texture fallback id={} meta={} key={:?} top={} east={} south={}",
                item_id,
                damage,
                block_registry_key(block_id),
                top_name,
                east_name,
                south_name
            );
        }
    }

    let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
    let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
    let cube_faces = [
        (
            IconQuad {
                vertices: [
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: format!("blocks/{south_name}"),
                tint_index: None,
            },
            south,
        ),
        (
            IconQuad {
                vertices: [
                    [1.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                ],
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: format!("blocks/{east_name}"),
                tint_index: None,
            },
            east,
        ),
        (
            IconQuad {
                vertices: [
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: format!("blocks/{top_name}"),
                tint_index: None,
            },
            top,
        ),
    ];
    for (quad, tex) in &cube_faces {
        raster_iso_quad(&mut out, &mut depth, quad, tex, [255, 255, 255]);
    }
    if logged_resolution_path.insert((item_id, damage)) {
        warn!(
            "[isometric-debug] path=id:{} meta:{} source:manual-cube-fallback key={:?}",
            item_id,
            damage,
            block_registry_key(block_id)
        );
    }
    Some(out)
}

fn load_block_texture(
    name: &str,
    cache: &mut HashMap<String, Option<egui::ColorImage>>,
) -> Option<egui::ColorImage> {
    load_model_texture(&format!("blocks/{name}"), cache)
}

fn fallback_block_face_texture(block_id: u16, damage: i16, face: BlockFace) -> Option<String> {
    let meta = damage as u8;
    let color = |m: u8| -> &'static str {
        match m & 0xF {
            0 => "white",
            1 => "orange",
            2 => "magenta",
            3 => "light_blue",
            4 => "yellow",
            5 => "lime",
            6 => "pink",
            7 => "gray",
            8 => "silver",
            9 => "cyan",
            10 => "purple",
            11 => "blue",
            12 => "brown",
            13 => "green",
            14 => "red",
            _ => "black",
        }
    };
    let wood = |m: u8| -> &'static str {
        match m & 0x7 {
            1 => "spruce",
            2 => "birch",
            3 => "jungle",
            4 => "acacia",
            5 => "big_oak",
            _ => "oak",
        }
    };
    match block_id {
        35 => Some(format!("wool_colored_{}.png", color(meta))),
        95 => Some(format!("glass_{}.png", color(meta))),
        159 => Some(format!("hardened_clay_stained_{}.png", color(meta))),
        160 => Some(format!("glass_{}.png", color(meta))),
        171 => Some(format!("wool_colored_{}.png", color(meta))),
        5 => Some(format!("planks_{}.png", wood(meta))),
        6 => {
            let sap = match meta & 0x7 {
                1 => "sapling_spruce",
                2 => "sapling_birch",
                3 => "sapling_jungle",
                4 => "sapling_acacia",
                5 => "sapling_roofed_oak",
                _ => "sapling_oak",
            };
            Some(format!("{sap}.png"))
        }
        17 => Some(match face {
            BlockFace::Up | BlockFace::Down => match meta & 0x3 {
                1 => "log_spruce_top.png".to_string(),
                2 => "log_birch_top.png".to_string(),
                3 => "log_jungle_top.png".to_string(),
                _ => "log_oak_top.png".to_string(),
            },
            _ => match meta & 0x3 {
                1 => "log_spruce.png".to_string(),
                2 => "log_birch.png".to_string(),
                3 => "log_jungle.png".to_string(),
                _ => "log_oak.png".to_string(),
            },
        }),
        18 => Some(match meta & 0x3 {
            1 => "leaves_spruce.png".to_string(),
            2 => "leaves_birch.png".to_string(),
            3 => "leaves_jungle.png".to_string(),
            _ => "leaves_oak.png".to_string(),
        }),
        161 => Some(match meta & 0x1 {
            1 => "leaves_big_oak.png".to_string(),
            _ => "leaves_acacia.png".to_string(),
        }),
        162 => Some(match face {
            BlockFace::Up | BlockFace::Down => match meta & 0x1 {
                1 => "log_big_oak_top.png".to_string(),
                _ => "log_acacia_top.png".to_string(),
            },
            _ => match meta & 0x1 {
                1 => "log_big_oak.png".to_string(),
                _ => "log_acacia.png".to_string(),
            },
        }),
        _ => None,
    }
}

fn quad_depth(quad: &IconQuad) -> f32 {
    let mut depth = 0.0;
    for v in &quad.vertices {
        depth += v[0] + v[1] + v[2];
    }
    depth / 4.0
}

fn load_model_texture(
    texture_path: &str,
    cache: &mut HashMap<String, Option<egui::ColorImage>>,
) -> Option<egui::ColorImage> {
    if let Some(cached) = cache.get(texture_path) {
        return cached.clone();
    }
    let path = texturepack_textures_root().join(texture_path);
    let image = load_color_image(&path);
    cache.insert(texture_path.to_string(), image.clone());
    image
}

fn raster_iso_quad(
    dst: &mut egui::ColorImage,
    depth: &mut [f32],
    quad: &IconQuad,
    tex: &egui::ColorImage,
    tint: [u8; 3],
) {
    let mut pts = [[0.0f32; 2]; 4];
    let mut z = [0.0f32; 4];
    for (i, v) in quad.vertices.iter().enumerate() {
        let [sx, sy, sz] = project_iso(*v);
        pts[i] = [sx, sy];
        z[i] = sz;
    }
    let shade = face_shade(quad);
    raster_textured_triangle(
        dst,
        depth,
        tex,
        pts[0],
        pts[1],
        pts[2],
        z[0],
        z[1],
        z[2],
        quad.uv[0],
        quad.uv[1],
        quad.uv[2],
        shade,
        tint,
    );
    raster_textured_triangle(
        dst,
        depth,
        tex,
        pts[0],
        pts[2],
        pts[3],
        z[0],
        z[2],
        z[3],
        quad.uv[0],
        quad.uv[2],
        quad.uv[3],
        shade,
        tint,
    );
}

fn project_iso(v: [f32; 3]) -> [f32; 3] {
    let x = v[0] - 0.5;
    let y = v[1] - 0.5;
    let z = v[2] - 0.5;
    let sx = (x - z) * 24.0 + 24.0;
    let sy = ((x + z) * 12.0 - y * 24.0) + 26.0;
    let sz = x + y + z;
    [sx, sy, sz]
}

fn face_shade(quad: &IconQuad) -> f32 {
    let a = quad.vertices[0];
    let b = quad.vertices[1];
    let c = quad.vertices[2];
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len <= f32::EPSILON {
        return 1.0;
    }
    let n = [n[0] / len, n[1] / len, n[2] / len];
    if n[1].abs() > 0.8 {
        return 1.0;
    }
    if n[0] > 0.35 {
        return 0.82;
    }
    if n[2] > 0.35 {
        return 0.66;
    }
    if n[0] < -0.35 || n[2] < -0.35 {
        return 0.58;
    }
    0.72
}

fn raster_textured_triangle(
    dst: &mut egui::ColorImage,
    depth: &mut [f32],
    tex: &egui::ColorImage,
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    z0: f32,
    z1: f32,
    z2: f32,
    uv0: [f32; 2],
    uv1: [f32; 2],
    uv2: [f32; 2],
    shade: f32,
    tint: [u8; 3],
) {
    let min_x = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as i32;
    let max_x = p0[0]
        .max(p1[0])
        .max(p2[0])
        .ceil()
        .min((dst.size[0] - 1) as f32) as i32;
    let min_y = p0[1].min(p1[1]).min(p2[1]).floor().max(0.0) as i32;
    let max_y = p0[1]
        .max(p1[1])
        .max(p2[1])
        .ceil()
        .min((dst.size[1] - 1) as f32) as i32;
    let area = edge_fn(p0, p1, p2);
    if area.abs() < 1e-5 {
        return;
    }

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let w0 = edge_fn(p1, p2, p) / area;
            let w1 = edge_fn(p2, p0, p) / area;
            let w2 = edge_fn(p0, p1, p) / area;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }
            let z = w0 * z0 + w1 * z1 + w2 * z2;
            let idx = y as usize * dst.size[0] + x as usize;
            if z <= depth[idx] {
                continue;
            }
            let u = w0 * uv0[0] + w1 * uv1[0] + w2 * uv2[0];
            let v = w0 * uv0[1] + w1 * uv1[1] + w2 * uv2[1];
            let tx = (u.clamp(0.0, 1.0) * (tex.size[0] as f32 - 1.0)).round() as usize;
            let ty = (v.clamp(0.0, 1.0) * (tex.size[1] as f32 - 1.0)).round() as usize;
            let mut c = tex.pixels[ty * tex.size[0] + tx];
            if c.a() == 0 {
                continue;
            }
            c = egui::Color32::from_rgba_unmultiplied(
                (f32::from(c.r()) * shade * (f32::from(tint[0]) / 255.0)).clamp(0.0, 255.0) as u8,
                (f32::from(c.g()) * shade * (f32::from(tint[1]) / 255.0)).clamp(0.0, 255.0) as u8,
                (f32::from(c.b()) * shade * (f32::from(tint[2]) / 255.0)).clamp(0.0, 255.0) as u8,
                c.a(),
            );
            dst.pixels[idx] = c;
            depth[idx] = z;
        }
    }
}

fn icon_tint_color(block_id: u16, damage: i16) -> Option<[u8; 3]> {
    // Deterministic inventory/debug tints approximating vanilla biome coloring.
    // This avoids grayscale foliage/grass in icon views.
    let meta = damage as u8;
    match block_id {
        // Grass family
        2 | 31 | 59 | 83 => Some([0x7f, 0xb2, 0x38]),
        // Vines / foliage family
        106 | 111 | 161 => Some([0x48, 0xb5, 0x18]),
        18 => Some(match meta & 0x3 {
            1 => [0x61, 0x99, 0x61], // spruce
            2 => [0x80, 0xA7, 0x55], // birch
            _ => [0x48, 0xB5, 0x18], // oak/jungle
        }),
        175 => match meta & 0x7 {
            2 | 3 => Some([0x7f, 0xb2, 0x38]), // double grass / fern
            _ => None,
        },
        // Water-like tint
        8 | 9 => Some([0x3f, 0x76, 0xe4]),
        _ => None,
    }
}

fn edge_fn(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}

fn texturepack_textures_root() -> PathBuf {
    rs_utils::texturepack_textures_root()
}

fn load_color_image(path: &Path) -> Option<egui::ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let mut rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    // For animated texture sheets (e.g. frame stacks), use first frame only.
    // Vanilla atlas animation advances frames over time; debug icons should not stretch the full sheet.
    if rgba.height() > rgba.width() && rgba.height() % rgba.width() == 0 {
        let w = rgba.width();
        let frame = image::imageops::crop(&mut rgba, 0, 0, w, w).to_image();
        rgba = frame;
    }
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}
