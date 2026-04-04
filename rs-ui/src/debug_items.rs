use super::*;
use crate::item_icons::ItemIconCache;
use crate::inventory_ui::{draw_inventory_item_tooltip, draw_slot};

pub(crate) fn build_debug_item_list() -> Vec<InventoryItemStack> {
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

pub(crate) fn draw_debug_item_browser(
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
                                true,
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
