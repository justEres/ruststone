use std::collections::HashMap;

use bevy::color::LinearRgba;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use rs_utils::{InventoryItemStack, InventoryState};

use crate::model::{
    BIPED_ARMOR_INNER_MODEL, BIPED_ARMOR_OUTER_MODEL, BIPED_BODY, BIPED_HEAD, BIPED_HEADWEAR,
    BIPED_LEFT_ARM, BIPED_LEFT_LEG, BIPED_RIGHT_ARM, BIPED_RIGHT_LEG, EntityTextureCache,
    part_mesh_with_front_back_swap,
};

const GLINT_TEXTURE_PATH: &str = "misc/enchanted_item_glint.png";

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanoidRigKind {
    Player,
    BipedMob,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct HumanoidRigParts {
    pub kind: HumanoidRigKind,
    pub model_root: Entity,
    pub head: Entity,
    pub body: Entity,
    pub arm_right: Entity,
    pub arm_left: Entity,
    pub leg_right: Entity,
    pub leg_left: Entity,
    pub render_layer: Option<usize>,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct HumanoidArmorState {
    pub boots: Option<InventoryItemStack>,
    pub leggings: Option<InventoryItemStack>,
    pub chestplate: Option<InventoryItemStack>,
    pub helmet: Option<InventoryItemStack>,
}

#[derive(Component, Debug, Default)]
pub struct HumanoidArmorLayerEntities {
    pub boots: Option<ArmorPieceSpawn>,
    pub leggings: Option<ArmorPieceSpawn>,
    pub chestplate: Option<ArmorPieceSpawn>,
    pub helmet: Option<ArmorPieceSpawn>,
}

#[derive(Debug)]
pub struct ArmorPieceSpawn {
    pub key: ArmorPieceRenderKey,
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmorSlot {
    Boots,
    Leggings,
    Chestplate,
    Helmet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmorMaterialKind {
    Leather,
    Chainmail,
    Iron,
    Gold,
    Diamond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmorPiece {
    pub slot: ArmorSlot,
    pub material: ArmorMaterialKind,
    pub leather_color: Option<u32>,
    pub enchanted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArmorPieceRenderKey {
    pub slot: ArmorSlot,
    pub material: ArmorMaterialKind,
    pub leather_color: Option<u32>,
    pub enchanted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ArmorMaterialKey {
    path: &'static str,
    tint: Option<u32>,
    glint: bool,
}

#[derive(Debug, Clone)]
struct ArmorMaterialSet {
    base: Handle<StandardMaterial>,
    overlay: Option<Handle<StandardMaterial>>,
    glint: Option<Handle<StandardMaterial>>,
}

#[derive(Resource, Default)]
pub struct ArmorTextureCache {
    materials: HashMap<ArmorMaterialKey, ArmorMaterialSet>,
}

pub fn sync_local_player_armor_state_system(
    inventory: Res<InventoryState>,
    mut query: Query<&mut HumanoidArmorState, With<crate::LocalPlayerModel>>,
) {
    let desired = HumanoidArmorState {
        helmet: inventory.player_slots.get(5).cloned().flatten(),
        chestplate: inventory.player_slots.get(6).cloned().flatten(),
        leggings: inventory.player_slots.get(7).cloned().flatten(),
        boots: inventory.player_slots.get(8).cloned().flatten(),
    };

    for mut state in &mut query {
        if *state != desired {
            *state = desired.clone();
        }
    }
}

pub fn reconcile_humanoid_armor_layers_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut entity_textures: ResMut<EntityTextureCache>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<ArmorTextureCache>,
    mut query: Query<(&HumanoidRigParts, &HumanoidArmorState, &mut HumanoidArmorLayerEntities)>,
) {
    for (rig, state, mut layers) in &mut query {
        reconcile_slot(
            &mut commands,
            &mut meshes,
            &mut entity_textures,
            &mut materials,
            &mut cache,
            rig,
            &state.boots,
            ArmorSlot::Boots,
            &mut layers.boots,
        );
        reconcile_slot(
            &mut commands,
            &mut meshes,
            &mut entity_textures,
            &mut materials,
            &mut cache,
            rig,
            &state.leggings,
            ArmorSlot::Leggings,
            &mut layers.leggings,
        );
        reconcile_slot(
            &mut commands,
            &mut meshes,
            &mut entity_textures,
            &mut materials,
            &mut cache,
            rig,
            &state.chestplate,
            ArmorSlot::Chestplate,
            &mut layers.chestplate,
        );
        reconcile_slot(
            &mut commands,
            &mut meshes,
            &mut entity_textures,
            &mut materials,
            &mut cache,
            rig,
            &state.helmet,
            ArmorSlot::Helmet,
            &mut layers.helmet,
        );
    }
}

fn reconcile_slot(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    entity_textures: &mut EntityTextureCache,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut ArmorTextureCache,
    rig: &HumanoidRigParts,
    stack: &Option<InventoryItemStack>,
    _slot: ArmorSlot,
    existing: &mut Option<ArmorPieceSpawn>,
) {
    let desired = stack.as_ref().and_then(classify_armor_piece);
    let desired_key = desired.map(|piece| ArmorPieceRenderKey {
        slot: piece.slot,
        material: piece.material,
        leather_color: piece.leather_color,
        enchanted: piece.enchanted,
    });

    if existing.as_ref().map(|piece| piece.key) == desired_key {
        return;
    }

    if let Some(existing_piece) = existing.take() {
        for entity in existing_piece.entities {
            commands.entity(entity).despawn_recursive();
        }
    }

    let Some(piece) = desired else {
        return;
    };

    let Some(spawned) = spawn_armor_piece(
        commands,
        meshes,
        entity_textures,
        materials,
        cache,
        rig,
        piece,
    ) else {
        return;
    };
    *existing = Some(spawned);
}

fn spawn_armor_piece(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    entity_textures: &mut EntityTextureCache,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut ArmorTextureCache,
    rig: &HumanoidRigParts,
    piece: ArmorPiece,
) -> Option<ArmorPieceSpawn> {
    let model = match piece.slot {
        ArmorSlot::Leggings => &BIPED_ARMOR_INNER_MODEL,
        ArmorSlot::Boots | ArmorSlot::Chestplate | ArmorSlot::Helmet => &BIPED_ARMOR_OUTER_MODEL,
    };
    let material_set = cache.materials_for(entity_textures, materials, piece)?;
    let mut entities = Vec::new();
    let swap_front_back = rig.kind == HumanoidRigKind::Player;

    for &part_index in visible_parts(piece.slot) {
        let parent = match part_index {
            BIPED_HEAD | BIPED_HEADWEAR => rig.head,
            BIPED_BODY => rig.body,
            BIPED_RIGHT_ARM => rig.arm_right,
            BIPED_LEFT_ARM => rig.arm_left,
            BIPED_RIGHT_LEG => rig.leg_right,
            BIPED_LEFT_LEG => rig.leg_left,
            _ => continue,
        };

        let mut anchor = commands.spawn((
            Name::new(format!("HumanoidArmorAnchor[{part_index}]")),
            Transform::IDENTITY,
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
        if let Some(layer) = rig.render_layer {
            anchor.insert(RenderLayers::layer(layer));
        }
        let anchor = anchor.id();
        commands.entity(parent).add_child(anchor);
        entities.push(anchor);

        let mesh = meshes.add(part_mesh_with_front_back_swap(
            model,
            &model.parts[part_index],
            swap_front_back,
        ));
        spawn_armor_mesh(commands, rig.render_layer, anchor, mesh.clone(), material_set.base.clone());
        if let Some(overlay) = &material_set.overlay {
            spawn_armor_mesh(commands, rig.render_layer, anchor, mesh.clone(), overlay.clone());
        }
        if let Some(glint) = &material_set.glint {
            spawn_armor_mesh(commands, rig.render_layer, anchor, mesh, glint.clone());
        }
    }

    Some(ArmorPieceSpawn {
        key: ArmorPieceRenderKey {
            slot: piece.slot,
            material: piece.material,
            leather_color: piece.leather_color,
            enchanted: piece.enchanted,
        },
        entities,
    })
}

fn spawn_armor_mesh(
    commands: &mut Commands,
    render_layer: Option<usize>,
    parent: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
) {
    let mut entity = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        GlobalTransform::default(),
        Visibility::Inherited,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));
    if let Some(layer) = render_layer {
        entity.insert(RenderLayers::layer(layer));
    }
    let entity = entity.id();
    commands.entity(parent).add_child(entity);
}

impl ArmorTextureCache {
    fn materials_for(
        &mut self,
        entity_textures: &mut EntityTextureCache,
        materials: &mut Assets<StandardMaterial>,
        piece: ArmorPiece,
    ) -> Option<ArmorMaterialSet> {
        let path = armor_texture_path(piece.material, piece.slot, false);
        let key = ArmorMaterialKey {
            path,
            tint: piece.leather_color,
            glint: piece.enchanted,
        };
        if let Some(existing) = self.materials.get(&key) {
            return Some(existing.clone());
        }

        entity_textures.request(path);
        if piece.material == ArmorMaterialKind::Leather {
            entity_textures.request(armor_texture_path(piece.material, piece.slot, true));
        }
        if piece.enchanted {
            entity_textures.request(GLINT_TEXTURE_PATH);
        }

        let Some(base_texture) = entity_textures.texture(path) else {
            return None;
        };

        let base = materials.add(StandardMaterial {
            base_color: piece
                .leather_color
                .map(leather_color_to_bevy)
                .unwrap_or(Color::WHITE),
            base_color_texture: Some(base_texture),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..Default::default()
        });

        let overlay = if piece.material == ArmorMaterialKind::Leather {
            let overlay_path = armor_texture_path(piece.material, piece.slot, true);
            let Some(overlay_texture) = entity_textures.texture(overlay_path) else {
                return None;
            };
            Some(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(overlay_texture),
                alpha_mode: AlphaMode::Mask(0.5),
                unlit: true,
                perceptual_roughness: 1.0,
                metallic: 0.0,
                ..Default::default()
            }))
        } else {
            None
        };

        let glint = if piece.enchanted {
            let Some(glint_texture) = entity_textures.texture(GLINT_TEXTURE_PATH) else {
                return None;
            };
            Some(materials.add(StandardMaterial {
                base_color: Color::srgba(0.38, 0.19, 0.70, 0.45),
                emissive: LinearRgba::rgb(0.38, 0.19, 0.70),
                base_color_texture: Some(glint_texture),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                perceptual_roughness: 1.0,
                metallic: 0.0,
                ..Default::default()
            }))
        } else {
            None
        };

        let set = ArmorMaterialSet {
            base,
            overlay,
            glint,
        };
        self.materials.insert(key, set.clone());
        Some(set)
    }
}

fn visible_parts(slot: ArmorSlot) -> &'static [usize] {
    match slot {
        ArmorSlot::Boots => &[BIPED_RIGHT_LEG, BIPED_LEFT_LEG],
        ArmorSlot::Leggings => &[BIPED_BODY, BIPED_RIGHT_LEG, BIPED_LEFT_LEG],
        ArmorSlot::Chestplate => &[BIPED_BODY, BIPED_RIGHT_ARM, BIPED_LEFT_ARM],
        ArmorSlot::Helmet => &[BIPED_HEAD, BIPED_HEADWEAR],
    }
}

fn armor_texture_path(
    material: ArmorMaterialKind,
    slot: ArmorSlot,
    overlay: bool,
) -> &'static str {
    match (material, slot, overlay) {
        (ArmorMaterialKind::Leather, ArmorSlot::Leggings, false) => {
            "models/armor/leather_layer_2.png"
        }
        (ArmorMaterialKind::Leather, ArmorSlot::Leggings, true) => {
            "models/armor/leather_layer_2_overlay.png"
        }
        (ArmorMaterialKind::Leather, _, false) => "models/armor/leather_layer_1.png",
        (ArmorMaterialKind::Leather, _, true) => "models/armor/leather_layer_1_overlay.png",
        (ArmorMaterialKind::Chainmail, ArmorSlot::Leggings, _) => {
            "models/armor/chainmail_layer_2.png"
        }
        (ArmorMaterialKind::Chainmail, _, _) => "models/armor/chainmail_layer_1.png",
        (ArmorMaterialKind::Iron, ArmorSlot::Leggings, _) => {
            "models/armor/iron_layer_2.png"
        }
        (ArmorMaterialKind::Iron, _, _) => "models/armor/iron_layer_1.png",
        (ArmorMaterialKind::Gold, ArmorSlot::Leggings, _) => "models/armor/gold_layer_2.png",
        (ArmorMaterialKind::Gold, _, _) => "models/armor/gold_layer_1.png",
        (ArmorMaterialKind::Diamond, ArmorSlot::Leggings, _) => {
            "models/armor/diamond_layer_2.png"
        }
        (ArmorMaterialKind::Diamond, _, _) => "models/armor/diamond_layer_1.png",
    }
}

fn leather_color_to_bevy(color: u32) -> Color {
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    Color::srgb(r, g, b)
}

pub fn classify_armor_piece(stack: &InventoryItemStack) -> Option<ArmorPiece> {
    let (slot, material) = match stack.item_id {
        298 => (ArmorSlot::Helmet, ArmorMaterialKind::Leather),
        299 => (ArmorSlot::Chestplate, ArmorMaterialKind::Leather),
        300 => (ArmorSlot::Leggings, ArmorMaterialKind::Leather),
        301 => (ArmorSlot::Boots, ArmorMaterialKind::Leather),
        302 => (ArmorSlot::Helmet, ArmorMaterialKind::Chainmail),
        303 => (ArmorSlot::Chestplate, ArmorMaterialKind::Chainmail),
        304 => (ArmorSlot::Leggings, ArmorMaterialKind::Chainmail),
        305 => (ArmorSlot::Boots, ArmorMaterialKind::Chainmail),
        306 => (ArmorSlot::Helmet, ArmorMaterialKind::Iron),
        307 => (ArmorSlot::Chestplate, ArmorMaterialKind::Iron),
        308 => (ArmorSlot::Leggings, ArmorMaterialKind::Iron),
        309 => (ArmorSlot::Boots, ArmorMaterialKind::Iron),
        310 => (ArmorSlot::Helmet, ArmorMaterialKind::Diamond),
        311 => (ArmorSlot::Chestplate, ArmorMaterialKind::Diamond),
        312 => (ArmorSlot::Leggings, ArmorMaterialKind::Diamond),
        313 => (ArmorSlot::Boots, ArmorMaterialKind::Diamond),
        314 => (ArmorSlot::Helmet, ArmorMaterialKind::Gold),
        315 => (ArmorSlot::Chestplate, ArmorMaterialKind::Gold),
        316 => (ArmorSlot::Leggings, ArmorMaterialKind::Gold),
        317 => (ArmorSlot::Boots, ArmorMaterialKind::Gold),
        _ => return None,
    };
    Some(ArmorPiece {
        slot,
        material,
        leather_color: if material == ArmorMaterialKind::Leather {
            stack.meta.display_color
        } else {
            None
        },
        enchanted: !stack.meta.enchantments.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_utils::InventoryItemMeta;

    fn stack(item_id: i32) -> InventoryItemStack {
        InventoryItemStack {
            item_id,
            count: 1,
            damage: 0,
            meta: InventoryItemMeta::default(),
        }
    }

    #[test]
    fn classifies_armor_material_and_slot() {
        let leggings = classify_armor_piece(&stack(308)).unwrap();
        assert_eq!(leggings.slot, ArmorSlot::Leggings);
        assert_eq!(leggings.material, ArmorMaterialKind::Iron);
    }

    #[test]
    fn maps_leggings_to_layer_two_texture() {
        assert_eq!(
            armor_texture_path(ArmorMaterialKind::Diamond, ArmorSlot::Leggings, false),
            "models/armor/diamond_layer_2.png"
        );
    }

    #[test]
    fn maps_leather_overlay_texture() {
        assert_eq!(
            armor_texture_path(ArmorMaterialKind::Leather, ArmorSlot::Helmet, true),
            "models/armor/leather_layer_1_overlay.png"
        );
    }

    #[test]
    fn local_inventory_slots_map_to_vanilla_armor_order() {
        let mut inventory = InventoryState::default();
        inventory.player_slots = vec![
            None,
            None,
            None,
            None,
            None,
            Some(stack(298)),
            Some(stack(307)),
            Some(stack(316)),
            Some(stack(313)),
        ];
        let state = HumanoidArmorState {
            helmet: inventory.player_slots.get(5).cloned().flatten(),
            chestplate: inventory.player_slots.get(6).cloned().flatten(),
            leggings: inventory.player_slots.get(7).cloned().flatten(),
            boots: inventory.player_slots.get(8).cloned().flatten(),
        };
        assert_eq!(classify_armor_piece(state.helmet.as_ref().unwrap()).unwrap().slot, ArmorSlot::Helmet);
        assert_eq!(classify_armor_piece(state.chestplate.as_ref().unwrap()).unwrap().slot, ArmorSlot::Chestplate);
        assert_eq!(classify_armor_piece(state.leggings.as_ref().unwrap()).unwrap().slot, ArmorSlot::Leggings);
        assert_eq!(classify_armor_piece(state.boots.as_ref().unwrap()).unwrap().slot, ArmorSlot::Boots);
    }

    #[test]
    fn slot_visibility_matches_vanilla() {
        assert_eq!(visible_parts(ArmorSlot::Boots), &[BIPED_RIGHT_LEG, BIPED_LEFT_LEG]);
        assert_eq!(visible_parts(ArmorSlot::Helmet), &[BIPED_HEAD, BIPED_HEADWEAR]);
    }
}
