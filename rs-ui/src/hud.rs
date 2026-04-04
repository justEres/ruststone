use super::*;
use crate::item_icons::ItemIconCache;
use crate::inventory_ui::draw_slot;

pub(crate) fn draw_hotbar_ui(
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
