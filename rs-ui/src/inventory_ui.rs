use super::*;
use crate::inventory_interaction::{
    finish_inventory_drag_if_released, handle_inventory_slot_interaction, send_inventory_click,
};
use crate::item_icons::ItemIconCache;
use crate::tooltips::{draw_item_tooltip, item_short_label};

pub(crate) fn draw_inventory_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    state: &mut ConnectUiState,
    inventory_state: &mut InventoryState,
    item_icons: &mut ItemIconCache,
) {
    if let Some(window) = inventory_state
        .open_window
        .clone()
        .filter(|window| window.id != 0)
    {
        draw_container_inventory_grid(
            ctx,
            ui,
            to_net,
            keys,
            state,
            inventory_state,
            item_icons,
            &window,
        );
        finish_inventory_drag_if_released(ctx, to_net, state, inventory_state);
        return;
    }

    draw_player_inventory_grid(ctx, ui, to_net, keys, state, inventory_state, item_icons);
    finish_inventory_drag_if_released(ctx, to_net, state, inventory_state);
}

pub(crate) fn draw_player_inventory_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    state: &mut ConnectUiState,
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
                    ctx,
                    response,
                    0,
                    0,
                    slot as i16,
                    keys,
                    to_net,
                    state,
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
                        ctx,
                        response,
                        0,
                        0,
                        slot as i16,
                        keys,
                        to_net,
                        state,
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
                    ctx,
                    response,
                    0,
                    0,
                    slot,
                    keys,
                    to_net,
                    state,
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

pub(crate) fn draw_container_inventory_grid(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    to_net: &ToNet,
    keys: &ButtonInput<KeyCode>,
    state: &mut ConnectUiState,
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
                            ctx,
                            response,
                            window.id,
                            unique_slots,
                            slot as i16,
                            keys,
                            to_net,
                            state,
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
                        ctx,
                        response,
                        window.id,
                        unique_slots,
                        slot as i16,
                        keys,
                        to_net,
                        state,
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
                    ctx,
                    response,
                    window.id,
                    unique_slots,
                    slot as i16,
                    keys,
                    to_net,
                    state,
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

pub(crate) fn draw_slot(
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

pub(crate) fn draw_inventory_item_tooltip(ctx: &egui::Context, stack: &InventoryItemStack) {
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
