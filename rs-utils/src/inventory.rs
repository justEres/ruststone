use std::collections::HashMap;

use bevy::ecs::resource::Resource;
use crate::block_registry_key;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEnchantment {
    pub id: i16,
    pub level: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryItemMeta {
    pub display_name: Option<String>,
    pub lore: Vec<String>,
    pub enchantments: Vec<InventoryEnchantment>,
    pub repair_cost: Option<i32>,
    pub unbreakable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItemStack {
    pub item_id: i32,
    pub count: u8,
    pub damage: i16,
    pub meta: InventoryItemMeta,
}

#[derive(Debug, Clone)]
pub struct InventoryWindowInfo {
    pub id: u8,
    pub kind: String,
    pub title: String,
    pub slot_count: u8,
}

#[derive(Debug, Clone)]
pub enum InventoryMessage {
    WindowOpen(InventoryWindowInfo),
    WindowClose {
        id: u8,
    },
    WindowItems {
        id: u8,
        items: Vec<Option<InventoryItemStack>>,
    },
    WindowSetSlot {
        id: i8,
        slot: i16,
        item: Option<InventoryItemStack>,
    },
    ConfirmTransaction {
        id: u8,
        action_number: i16,
        accepted: bool,
    },
    SetCurrentHotbarSlot {
        slot: u8,
    },
}

#[derive(Resource, Debug, Default, Clone)]
pub struct InventoryState {
    pub player_slots: Vec<Option<InventoryItemStack>>,
    pub open_window: Option<InventoryWindowInfo>,
    pub window_slots: HashMap<u8, Vec<Option<InventoryItemStack>>>,
    pub cursor_item: Option<InventoryItemStack>,
    pub selected_hotbar_slot: u8,
    pub next_action_number: u16,
    pub pending_confirm_acks: Vec<(u8, i16)>,
    pending_clicks: HashMap<(u8, u16), InventorySnapshot>,
    drag_state: Option<InventoryDragState>,
}

impl InventoryState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn queue_confirm_ack(&mut self, id: u8, action_number: i16) {
        self.pending_confirm_acks.push((id, action_number));
    }

    pub fn drain_confirm_acks(&mut self) -> Vec<(u8, i16)> {
        std::mem::take(&mut self.pending_confirm_acks)
    }

    pub fn set_window_items(&mut self, id: u8, items: Vec<Option<InventoryItemStack>>) {
        if id == 0 {
            self.player_slots = items;
            if let Some(window_id) = self.active_window_id() {
                self.sync_window_from_player_slots(window_id);
            }
        } else {
            self.window_slots.insert(id, items);
            self.sync_player_from_window_slots(id);
        }
    }

    pub fn set_slot(&mut self, id: i8, slot: i16, item: Option<InventoryItemStack>) {
        if id == -1 && slot == -1 {
            self.cursor_item = item;
            return;
        }
        if slot < 0 {
            return;
        }
        let slot = slot as usize;
        if id == 0 {
            if self.player_slots.len() <= slot {
                self.player_slots.resize(slot + 1, None);
            }
            self.player_slots[slot] = item;
            if let Some(window_id) = self.active_window_id() {
                self.sync_window_from_player_slots(window_id);
            }
            return;
        }
        if id > 0 {
            let window_id = id as u8;
            let slots = self.window_slots.entry(window_id).or_default();
            if slots.len() <= slot {
                slots.resize(slot + 1, None);
            }
            slots[slot] = item;
            self.sync_player_from_window_slot(window_id, slot);
        }
    }

    pub fn hotbar_slot_index(&self, hotbar_index: u8) -> Option<usize> {
        if hotbar_index > 8 {
            return None;
        }
        if self.player_slots.len() >= 45 {
            Some(36 + hotbar_index as usize)
        } else if self.player_slots.len() >= 9 {
            Some(self.player_slots.len() - 9 + hotbar_index as usize)
        } else {
            None
        }
    }

    pub fn hotbar_item(&self, hotbar_index: u8) -> Option<InventoryItemStack> {
        let idx = self.hotbar_slot_index(hotbar_index)?;
        self.player_slots.get(idx).cloned().flatten()
    }

    pub fn selected_hotbar_slot_index(&self) -> Option<usize> {
        self.hotbar_slot_index(self.selected_hotbar_slot)
    }

    pub fn set_selected_hotbar_slot(&mut self, slot: u8) {
        self.selected_hotbar_slot = slot.min(8);
    }

    pub fn selected_hotbar_item(&self) -> Option<InventoryItemStack> {
        self.hotbar_item(self.selected_hotbar_slot)
    }

    pub fn consume_selected_hotbar_one(&mut self) -> bool {
        self.consume_selected_hotbar(1)
    }

    pub fn predict_place_selected_hotbar(&mut self) -> bool {
        let Some(stack) = self.selected_hotbar_item() else {
            return false;
        };
        if !is_placeable_block_item(stack.item_id) {
            return false;
        }
        self.consume_selected_hotbar(1)
    }

    pub fn predict_drop_selected_hotbar(&mut self, full_stack: bool) -> bool {
        let Some(idx) = self.selected_hotbar_slot_index() else {
            return false;
        };
        let Some(Some(mut stack)) = self.player_slots.get(idx).cloned() else {
            return false;
        };
        if stack.count == 0 {
            return false;
        }
        if full_stack || stack.count <= 1 {
            self.player_slots[idx] = None;
        } else {
            stack.count = stack.count.saturating_sub(1);
            self.player_slots[idx] = Some(stack);
        }
        true
    }

    fn consume_selected_hotbar(&mut self, amount: u8) -> bool {
        let Some(idx) = self.hotbar_slot_index(self.selected_hotbar_slot) else {
            return false;
        };
        let Some(Some(mut stack)) = self.player_slots.get(idx).cloned() else {
            return false;
        };
        if stack.count == 0 {
            return false;
        }
        stack.count = stack.count.saturating_sub(amount);
        self.player_slots[idx] = if stack.count == 0 { None } else { Some(stack) };
        true
    }

    pub fn apply_local_click_window(
        &mut self,
        window_id: u8,
        window_unique_slots: usize,
        slot: i16,
        button: u8,
        mode: u8,
    ) -> Option<InventoryItemStack> {
        let snapshot = self.snapshot();
        let action_number = self.next_action_number;
        self.pending_clicks.insert((window_id, action_number), snapshot);

        let mut slots = if window_id == 0 {
            self.player_slots.clone()
        } else {
            self.window_slots
                .get(&window_id)
                .cloned()
                .unwrap_or_default()
        };
        let mut cursor = self.cursor_item.clone();

        match mode {
            0 => apply_mode_normal_click(&mut slots, &mut cursor, slot, button),
            1 => apply_mode_shift_click(&mut slots, &cursor, window_unique_slots, slot),
            2 => apply_mode_number_key(&mut slots, &cursor, slot, button),
            4 => apply_mode_drop(&mut slots, &cursor, slot, button),
            5 => apply_mode_drag_paint(
                &mut slots,
                &mut cursor,
                &mut self.drag_state,
                window_id,
                slot,
                button,
            ),
            6 => apply_mode_double_click(&mut slots, &mut cursor, slot, button),
            _ => {}
        }
        let clicked_item = slot_item_after_click(&slots, slot);

        self.cursor_item = cursor;
        if window_id == 0 {
            self.player_slots = slots;
            if let Some(active_window_id) = self.active_window_id() {
                self.sync_window_from_player_slots(active_window_id);
            }
        } else {
            self.window_slots.insert(window_id, slots);
            self.sync_player_from_window_slots(window_id);
        }
        clicked_item
    }

    pub fn apply_transaction_result(&mut self, id: u8, action_number: i16, accepted: bool) {
        let Ok(action_number) = u16::try_from(action_number.max(0)) else {
            return;
        };
        if !accepted {
            if let Some(snapshot) = self.pending_clicks.get(&(id, action_number)).cloned() {
                self.restore_snapshot(snapshot);
            }
            self.pending_clicks
                .retain(|(window_id, tx), _| *window_id != id || *tx < action_number);
            return;
        }
        self.pending_clicks
            .retain(|(window_id, tx), _| *window_id != id || *tx > action_number);
    }

    fn snapshot(&self) -> InventorySnapshot {
        InventorySnapshot {
            player_slots: self.player_slots.clone(),
            window_slots: self.window_slots.clone(),
            cursor_item: self.cursor_item.clone(),
            open_window: self.open_window.clone(),
            selected_hotbar_slot: self.selected_hotbar_slot,
            drag_state: self.drag_state.clone(),
        }
    }

    fn restore_snapshot(&mut self, snapshot: InventorySnapshot) {
        self.player_slots = snapshot.player_slots;
        self.window_slots = snapshot.window_slots;
        self.cursor_item = snapshot.cursor_item;
        self.open_window = snapshot.open_window;
        self.selected_hotbar_slot = snapshot.selected_hotbar_slot;
        self.drag_state = snapshot.drag_state;
    }

    fn active_window_id(&self) -> Option<u8> {
        self.open_window
            .as_ref()
            .filter(|window| window.id != 0)
            .map(|window| window.id)
    }

    fn active_window_unique_slots(&self, window_id: u8) -> Option<usize> {
        self.open_window
            .as_ref()
            .filter(|window| window.id == window_id)
            .map(|window| window.slot_count as usize)
    }

    fn sync_player_from_window_slots(&mut self, window_id: u8) {
        let Some(unique_slots) = self.active_window_unique_slots(window_id) else {
            return;
        };
        let Some(slots) = self.window_slots.get(&window_id) else {
            return;
        };
        if slots.len() < unique_slots + 36 {
            return;
        }
        if self.player_slots.len() < 45 {
            self.player_slots.resize(45, None);
        }
        for player_offset in 0..36usize {
            let player_slot = if player_offset < 27 {
                9 + player_offset
            } else {
                36 + (player_offset - 27)
            };
            self.player_slots[player_slot] = slots[unique_slots + player_offset].clone();
        }
    }

    fn sync_player_from_window_slot(&mut self, window_id: u8, slot_index: usize) {
        let Some(unique_slots) = self.active_window_unique_slots(window_id) else {
            return;
        };
        if slot_index < unique_slots {
            return;
        }
        let player_offset = slot_index - unique_slots;
        if player_offset >= 36 {
            return;
        }
        let Some(slots) = self.window_slots.get(&window_id) else {
            return;
        };
        let item = slots.get(slot_index).cloned().flatten();
        if self.player_slots.len() < 45 {
            self.player_slots.resize(45, None);
        }
        let player_slot = if player_offset < 27 {
            9 + player_offset
        } else {
            36 + (player_offset - 27)
        };
        self.player_slots[player_slot] = item;
    }

    fn sync_window_from_player_slots(&mut self, window_id: u8) {
        let Some(unique_slots) = self.active_window_unique_slots(window_id) else {
            return;
        };
        let Some(slots) = self.window_slots.get_mut(&window_id) else {
            return;
        };
        if slots.len() < unique_slots + 36 || self.player_slots.len() < 45 {
            return;
        }
        for player_offset in 0..36usize {
            let player_slot = if player_offset < 27 {
                9 + player_offset
            } else {
                36 + (player_offset - 27)
            };
            slots[unique_slots + player_offset] = self.player_slots[player_slot].clone();
        }
    }

    pub fn apply_local_click_player_window(
        &mut self,
        slot: i16,
        button: u8,
        mode: u8,
    ) -> Option<InventoryItemStack> {
        self.apply_local_click_window(0, 0, slot, button, mode)
    }
}

#[derive(Debug, Clone)]
struct InventorySnapshot {
    player_slots: Vec<Option<InventoryItemStack>>,
    window_slots: HashMap<u8, Vec<Option<InventoryItemStack>>>,
    cursor_item: Option<InventoryItemStack>,
    open_window: Option<InventoryWindowInfo>,
    selected_hotbar_slot: u8,
    drag_state: Option<InventoryDragState>,
}

#[derive(Debug, Clone)]
struct InventoryDragState {
    window_id: u8,
    button: u8,
    original_cursor: InventoryItemStack,
    visited_slots: Vec<usize>,
    original_slots: HashMap<usize, Option<InventoryItemStack>>,
}

fn apply_outside_click(cursor_item: &mut Option<InventoryItemStack>, button: u8) {
    match cursor_item.clone() {
        Some(_) if button == 0 => {
            *cursor_item = None;
        }
        Some(mut cursor) if button == 1 => {
            cursor.count = cursor.count.saturating_sub(1);
            *cursor_item = if cursor.count == 0 {
                None
            } else {
                Some(cursor)
            };
        }
        _ => {}
    }
}

fn apply_mode_normal_click(
    slots: &mut Vec<Option<InventoryItemStack>>,
    cursor_item: &mut Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if slot < 0 {
        apply_outside_click(cursor_item, button);
        return;
    }

    let slot_index = slot as usize;
    if slots.len() <= slot_index {
        slots.resize(slot_index + 1, None);
    }

    let mut slot_item = slots[slot_index].clone();
    let mut cursor = cursor_item.clone();

    if button == 0 {
        match (cursor.clone(), slot_item.clone()) {
            (None, Some(_)) => {
                cursor = slot_item;
                slot_item = None;
            }
            (Some(_), None) => {
                slot_item = cursor;
                cursor = None;
            }
            (Some(cur), Some(mut sl)) => {
                if can_stack(&cur, &sl) && sl.count < max_stack_for_item(sl.item_id) {
                    let max = max_stack_for_item(sl.item_id);
                    let space = max.saturating_sub(sl.count);
                    let moved = space.min(cur.count);
                    sl.count = sl.count.saturating_add(moved);
                    let remaining = cur.count.saturating_sub(moved);
                    slot_item = Some(sl);
                    cursor = if remaining == 0 {
                        None
                    } else {
                        Some(InventoryItemStack {
                            count: remaining,
                            ..cur
                        })
                    };
                } else {
                    slot_item = Some(cur);
                    cursor = Some(sl);
                }
            }
            _ => {}
        }
    } else if button == 1 {
        match (cursor.clone(), slot_item.clone()) {
            (None, Some(mut sl)) => {
                let take = (sl.count.saturating_add(1)) / 2;
                let remain = sl.count.saturating_sub(take);
                cursor = Some(InventoryItemStack {
                    count: take,
                    ..sl.clone()
                });
                slot_item = if remain == 0 {
                    None
                } else {
                    sl.count = remain;
                    Some(sl)
                };
            }
            (Some(mut cur), None) => {
                slot_item = Some(InventoryItemStack {
                    count: 1,
                    ..cur.clone()
                });
                cur.count = cur.count.saturating_sub(1);
                cursor = if cur.count == 0 { None } else { Some(cur) };
            }
            (Some(mut cur), Some(mut sl)) => {
                if can_stack(&cur, &sl) && sl.count < max_stack_for_item(sl.item_id) {
                    sl.count = sl.count.saturating_add(1);
                    cur.count = cur.count.saturating_sub(1);
                    slot_item = Some(sl);
                    cursor = if cur.count == 0 { None } else { Some(cur) };
                } else {
                    slot_item = Some(cur);
                    cursor = Some(sl);
                }
            }
            _ => {}
        }
    }

    slots[slot_index] = slot_item;
    *cursor_item = cursor;
}

fn apply_mode_shift_click(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &Option<InventoryItemStack>,
    window_unique_slots: usize,
    slot: i16,
) {
    if slot < 0 || slots.is_empty() {
        return;
    }
    let slot_index = slot as usize;
    if slot_index >= slots.len() {
        return;
    }
    let Some(mut moving) = slots[slot_index].take() else {
        return;
    };

    let total = slots.len();
    let player_base = total.saturating_sub(36);
    let player_main_start = player_base;
    let player_hotbar_start = player_base + 27;
    let player_end = total;
    let unique = window_unique_slots.min(player_base);

    let mut targets: Vec<std::ops::Range<usize>> = Vec::new();
    if slot_index < unique {
        targets.push(player_main_start..player_hotbar_start.min(player_end));
        targets.push(player_hotbar_start.min(player_end)..player_end);
    } else if (player_main_start..player_hotbar_start.min(player_end)).contains(&slot_index) {
        targets.push(player_hotbar_start.min(player_end)..player_end);
        if unique > 0 {
            targets.push(0..unique);
        }
    } else if (player_hotbar_start.min(player_end)..player_end).contains(&slot_index) {
        targets.push(player_main_start..player_hotbar_start.min(player_end));
        if unique > 0 {
            targets.push(0..unique);
        }
    }

    if targets.is_empty() {
        slots[slot_index] = Some(moving);
        return;
    }

    for range in &targets {
        for idx in range.clone() {
            if moving.count == 0 {
                break;
            }
            let Some(existing) = slots.get_mut(idx) else {
                continue;
            };
            if let Some(stack) = existing.as_mut() {
                if can_stack(stack, &moving) && stack.count < max_stack_for_item(stack.item_id) {
                    let max = max_stack_for_item(stack.item_id);
                    let space = max.saturating_sub(stack.count);
                    let moved = space.min(moving.count);
                    stack.count = stack.count.saturating_add(moved);
                    moving.count = moving.count.saturating_sub(moved);
                }
            }
        }
    }

    for range in targets {
        for idx in range {
            if moving.count == 0 {
                break;
            }
            let Some(existing) = slots.get_mut(idx) else {
                continue;
            };
            if existing.is_none() {
                *existing = Some(moving.clone());
                moving.count = 0;
            }
        }
    }

    if moving.count > 0 {
        slots[slot_index] = Some(moving);
    }
    let _ = cursor_item;
}

fn apply_mode_number_key(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if slot < 0 || button > 8 || slots.len() < 9 {
        return;
    }
    let slot_index = slot as usize;
    let hotbar_start = slots.len() - 9;
    let hotbar_index = hotbar_start + button as usize;
    if slot_index >= slots.len() || hotbar_index >= slots.len() {
        return;
    }
    slots.swap(slot_index, hotbar_index);
    let _ = cursor_item;
}

fn apply_mode_drop(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if slot < 0 {
        return;
    }
    let slot_index = slot as usize;
    if slot_index >= slots.len() {
        return;
    }
    if let Some(mut stack) = slots[slot_index].clone() {
        if button == 0 {
            stack.count = stack.count.saturating_sub(1);
            slots[slot_index] = if stack.count == 0 { None } else { Some(stack) };
        } else {
            slots[slot_index] = None;
        }
    }
    let _ = cursor_item;
}

fn apply_mode_double_click(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &mut Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if button != 0 {
        return;
    }
    let mut cursor = match cursor_item.clone() {
        Some(c) => c,
        None => return,
    };
    let max = max_stack_for_item(cursor.item_id);
    if cursor.count >= max {
        return;
    }

    let skip_slot = if slot >= 0 { Some(slot as usize) } else { None };
    for idx in 0..slots.len() {
        if Some(idx) == skip_slot {
            continue;
        }
        let Some(mut stack) = slots[idx].clone() else {
            continue;
        };
        if !can_stack(&stack, &cursor) {
            continue;
        }
        let need = max.saturating_sub(cursor.count);
        if need == 0 {
            break;
        }
        let moved = need.min(stack.count);
        stack.count = stack.count.saturating_sub(moved);
        cursor.count = cursor.count.saturating_add(moved);
        slots[idx] = if stack.count == 0 { None } else { Some(stack) };
        if cursor.count >= max {
            break;
        }
    }
    *cursor_item = Some(cursor);
}

fn apply_mode_drag_paint(
    slots: &mut Vec<Option<InventoryItemStack>>,
    cursor_item: &mut Option<InventoryItemStack>,
    drag_state: &mut Option<InventoryDragState>,
    window_id: u8,
    slot: i16,
    button: u8,
) {
    match drag_button_phase(button) {
        Some((drag_button, DragPhase::Start)) => {
            let Some(cursor) = cursor_item.clone() else {
                return;
            };
            *drag_state = Some(InventoryDragState {
                window_id,
                button: drag_button,
                original_cursor: cursor,
                visited_slots: Vec::new(),
                original_slots: HashMap::new(),
            });
        }
        Some((drag_button, DragPhase::Add)) => {
            let Some(drag) = drag_state.as_mut() else {
                return;
            };
            if drag.window_id != window_id || drag.button != drag_button || slot < 0 {
                return;
            }
            let slot_index = slot as usize;
            if !drag.original_slots.contains_key(&slot_index) {
                if slots.len() <= slot_index {
                    slots.resize(slot_index + 1, None);
                }
                drag.original_slots.insert(slot_index, slots[slot_index].clone());
                drag.visited_slots.push(slot_index);
            }
            recompute_drag_distribution(slots, cursor_item, drag);
        }
        Some((drag_button, DragPhase::End)) => {
            let Some(drag) = drag_state.as_ref() else {
                return;
            };
            if drag.window_id == window_id && drag.button == drag_button {
                *drag_state = None;
            }
        }
        None => {}
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragPhase {
    Start,
    Add,
    End,
}

fn drag_button_phase(button: u8) -> Option<(u8, DragPhase)> {
    match button {
        0 => Some((0, DragPhase::Start)),
        1 => Some((0, DragPhase::Add)),
        2 => Some((0, DragPhase::End)),
        4 => Some((1, DragPhase::Start)),
        5 => Some((1, DragPhase::Add)),
        6 => Some((1, DragPhase::End)),
        _ => None,
    }
}

fn recompute_drag_distribution(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &mut Option<InventoryItemStack>,
    drag: &InventoryDragState,
) {
    for (slot_index, original) in &drag.original_slots {
        if *slot_index < slots.len() {
            slots[*slot_index] = original.clone();
        }
    }

    let mut remaining = drag.original_cursor.count;
    let mut placements = Vec::new();
    for slot_index in &drag.visited_slots {
        let capacity = match slots.get(*slot_index).cloned().flatten() {
            None => max_stack_for_item(drag.original_cursor.item_id),
            Some(slot_item) => {
                if !can_stack(&slot_item, &drag.original_cursor)
                    || slot_item.count >= max_stack_for_item(slot_item.item_id)
                {
                    0
                } else {
                    max_stack_for_item(slot_item.item_id).saturating_sub(slot_item.count)
                }
            }
        };
        placements.push((*slot_index, capacity));
    }

    if placements.is_empty() {
        *cursor_item = Some(drag.original_cursor.clone());
        return;
    }

    let target_count = placements
        .iter()
        .filter(|(_, capacity)| *capacity > 0)
        .count();
    if target_count == 0 {
        *cursor_item = Some(drag.original_cursor.clone());
        return;
    }

    let mut moved_total = 0u8;
    for (index, (slot_index, capacity)) in placements.iter().enumerate() {
        if *capacity == 0 || remaining == 0 {
            continue;
        }
        let wanted = if drag.button == 0 {
            let total = target_count as u8;
            let base = drag.original_cursor.count / total;
            let extra = drag.original_cursor.count % total;
            base.saturating_add(u8::from(index < extra as usize))
        } else {
            1
        };
        let moved = wanted.min(*capacity).min(remaining);
        if moved == 0 {
            continue;
        }

        let slot_item = slots[*slot_index].take();
        slots[*slot_index] = Some(match slot_item {
            Some(mut existing) => {
                existing.count = existing.count.saturating_add(moved);
                existing
            }
            None => InventoryItemStack {
                count: moved,
                ..drag.original_cursor.clone()
            },
        });
        remaining = remaining.saturating_sub(moved);
        moved_total = moved_total.saturating_add(moved);
    }

    if moved_total == 0 {
        *cursor_item = Some(drag.original_cursor.clone());
        return;
    }

    let mut cursor = drag.original_cursor.clone();
    cursor.count = drag.original_cursor.count.saturating_sub(moved_total);
    *cursor_item = if cursor.count == 0 { None } else { Some(cursor) };
}

fn can_stack(a: &InventoryItemStack, b: &InventoryItemStack) -> bool {
    a.item_id == b.item_id && a.damage == b.damage && a.meta == b.meta
}

fn slot_item_after_click(
    slots: &[Option<InventoryItemStack>],
    slot: i16,
) -> Option<InventoryItemStack> {
    if slot < 0 {
        return None;
    }
    slots.get(slot as usize).cloned().flatten()
}

fn max_stack_for_item(item_id: i32) -> u8 {
    if is_single_stack_item(item_id) { 1 } else { 64 }
}

fn is_placeable_block_item(item_id: i32) -> bool {
    if item_id <= 0 || item_id > u16::MAX as i32 {
        return false;
    }
    block_registry_key(item_id as u16).is_some()
}

#[derive(Clone, Copy)]
struct ItemProperties {
    durability: Option<i16>,
    single_stack: bool,
}

fn item_properties(item_id: i32) -> ItemProperties {
    match item_id {
        256 | 269 | 273 | 277 | 284 => ItemProperties {
            durability: Some(59),
            single_stack: true,
        },
        257 | 270 | 274 | 278 | 285 => ItemProperties {
            durability: Some(131),
            single_stack: true,
        },
        258 | 271 | 275 | 279 | 286 => ItemProperties {
            durability: Some(250),
            single_stack: true,
        },
        259 => ItemProperties {
            durability: Some(64),
            single_stack: true,
        },
        261 => ItemProperties {
            durability: Some(384),
            single_stack: true,
        },
        267 | 272 | 276 | 283 => ItemProperties {
            durability: Some(32),
            single_stack: true,
        },
        268 => ItemProperties {
            durability: Some(59),
            single_stack: true,
        },
        290 | 291 | 292 | 294 => ItemProperties {
            durability: Some(59),
            single_stack: true,
        },
        293 => ItemProperties {
            durability: Some(131),
            single_stack: true,
        },
        298 => ItemProperties {
            durability: Some(55),
            single_stack: true,
        },
        299 => ItemProperties {
            durability: Some(80),
            single_stack: true,
        },
        300 => ItemProperties {
            durability: Some(75),
            single_stack: true,
        },
        301 => ItemProperties {
            durability: Some(65),
            single_stack: true,
        },
        302 => ItemProperties {
            durability: Some(165),
            single_stack: true,
        },
        303 => ItemProperties {
            durability: Some(240),
            single_stack: true,
        },
        304 => ItemProperties {
            durability: Some(225),
            single_stack: true,
        },
        305 => ItemProperties {
            durability: Some(195),
            single_stack: true,
        },
        306 | 310 => ItemProperties {
            durability: Some(363),
            single_stack: true,
        },
        307 | 311 => ItemProperties {
            durability: Some(528),
            single_stack: true,
        },
        308 | 312 => ItemProperties {
            durability: Some(495),
            single_stack: true,
        },
        309 | 313 => ItemProperties {
            durability: Some(429),
            single_stack: true,
        },
        314 => ItemProperties {
            durability: Some(77),
            single_stack: true,
        },
        315 => ItemProperties {
            durability: Some(112),
            single_stack: true,
        },
        316 => ItemProperties {
            durability: Some(105),
            single_stack: true,
        },
        317 => ItemProperties {
            durability: Some(91),
            single_stack: true,
        },
        346 => ItemProperties {
            durability: Some(64),
            single_stack: true,
        },
        359 => ItemProperties {
            durability: Some(238),
            single_stack: true,
        },
        326..=330 => ItemProperties {
            durability: None,
            single_stack: true,
        },
        _ => ItemProperties {
            durability: None,
            single_stack: false,
        },
    }
}

pub fn item_max_durability(item_id: i32) -> Option<i16> {
    item_properties(item_id).durability
}

fn is_single_stack_item(item_id: i32) -> bool {
    item_properties(item_id).single_stack
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: i32, count: u8) -> InventoryItemStack {
        InventoryItemStack {
            item_id,
            count,
            damage: 0,
            meta: InventoryItemMeta::default(),
        }
    }

    #[test]
    fn predict_place_selected_hotbar_consumes_placeable_blocks() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            ..Default::default()
        };
        inventory.player_slots[36] = Some(stack(1, 3));
        inventory.set_selected_hotbar_slot(0);

        assert!(inventory.predict_place_selected_hotbar());
        assert_eq!(inventory.player_slots[36], Some(stack(1, 2)));
    }

    #[test]
    fn predict_place_selected_hotbar_does_not_consume_non_block_items() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            ..Default::default()
        };
        inventory.player_slots[36] = Some(stack(276, 1));
        inventory.set_selected_hotbar_slot(0);

        assert!(!inventory.predict_place_selected_hotbar());
        assert_eq!(inventory.player_slots[36], Some(stack(276, 1)));
    }

    #[test]
    fn predict_drop_selected_hotbar_updates_stack_count() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            ..Default::default()
        };
        inventory.player_slots[36] = Some(stack(4, 3));
        inventory.set_selected_hotbar_slot(0);

        assert!(inventory.predict_drop_selected_hotbar(false));
        assert_eq!(inventory.player_slots[36], Some(stack(4, 2)));
        assert!(inventory.predict_drop_selected_hotbar(true));
        assert_eq!(inventory.player_slots[36], None);
    }

    #[test]
    fn click_window_returns_post_click_slot_item() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            ..Default::default()
        };
        inventory.player_slots[0] = Some(stack(1, 8));

        let clicked_item = inventory.apply_local_click_player_window(0, 0, 0);

        assert_eq!(inventory.cursor_item, Some(stack(1, 8)));
        assert_eq!(clicked_item, None);
    }

    #[test]
    fn rejected_transaction_restores_snapshot() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            ..Default::default()
        };
        inventory.player_slots[0] = Some(stack(1, 8));

        let action_number = inventory.next_action_number;
        let _ = inventory.apply_local_click_player_window(0, 0, 0);
        inventory.next_action_number = inventory.next_action_number.wrapping_add(1);

        assert_eq!(inventory.cursor_item, Some(stack(1, 8)));
        assert_eq!(inventory.player_slots[0], None);

        inventory.apply_transaction_result(0, action_number as i16, false);

        assert_eq!(inventory.cursor_item, None);
        assert_eq!(inventory.player_slots[0], Some(stack(1, 8)));
    }

    #[test]
    fn left_drag_splits_evenly_across_empty_slots() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            cursor_item: Some(stack(1, 8)),
            ..Default::default()
        };

        let _ = inventory.apply_local_click_player_window(-999, 0, 5);
        let _ = inventory.apply_local_click_player_window(0, 1, 5);
        let _ = inventory.apply_local_click_player_window(1, 1, 5);
        let _ = inventory.apply_local_click_player_window(-999, 2, 5);

        assert_eq!(inventory.player_slots[0], Some(stack(1, 4)));
        assert_eq!(inventory.player_slots[1], Some(stack(1, 4)));
        assert_eq!(inventory.cursor_item, None);
    }

    #[test]
    fn right_drag_places_one_item_per_slot() {
        let mut inventory = InventoryState {
            player_slots: vec![None; 45],
            cursor_item: Some(stack(1, 5)),
            ..Default::default()
        };

        let _ = inventory.apply_local_click_player_window(-999, 4, 5);
        let _ = inventory.apply_local_click_player_window(0, 5, 5);
        let _ = inventory.apply_local_click_player_window(1, 5, 5);
        let _ = inventory.apply_local_click_player_window(-999, 6, 5);

        assert_eq!(inventory.player_slots[0], Some(stack(1, 1)));
        assert_eq!(inventory.player_slots[1], Some(stack(1, 1)));
        assert_eq!(inventory.cursor_item, Some(stack(1, 3)));
    }
}
