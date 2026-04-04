use super::*;

pub(crate) fn draw_item_tooltip(ui: &mut egui::Ui, stack: &InventoryItemStack) {
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

pub(crate) fn item_short_label(item_id: i32) -> &'static str {
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
