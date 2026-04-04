use std::collections::{HashMap, HashSet, VecDeque};
use std::thread;

mod armor;
mod components;
mod entity_anim_spawn;
mod first_person;
pub mod item_textures;
mod local_player;
mod motion;
pub mod model;
mod player_mesh;
mod registry;
mod remote_apply;
mod skins;
mod specs;

use bevy::color::LinearRgba;
use bevy::ecs::system::SystemParam;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy_egui::{EguiContexts, egui};
use crossbeam::channel::{Receiver, Sender, unbounded};
use rs_render::{
    CHUNK_CUTOUT_RENDER_LAYER, CHUNK_OPAQUE_RENDER_LAYER, CHUNK_TRANSPARENT_RENDER_LAYER,
    LOCAL_PLAYER_RENDER_LAYER, MAIN_RENDER_LAYER, PlayerCamera,
};
use rs_sim::collision::{WorldCollisionMap, is_solid};
use rs_utils::{
    AppState, ApplicationState, InventoryItemStack, MobKind, NetEntityAnimation, NetEntityKind,
    NetEntityMessage, PlayerSkinModel, UiState,
};
use tracing::{debug, info, warn};

use crate::armor::{
    HumanoidArmorLayerEntities, HumanoidArmorState, HumanoidRigKind, HumanoidRigParts,
};
pub use crate::armor::ArmorTextureCache;
pub use crate::armor::{reconcile_humanoid_armor_layers_system, sync_local_player_armor_state_system};
pub use crate::entity_anim_spawn::{
    animate_remote_biped_models, animate_remote_player_models, animate_remote_quadruped_models,
    rebuild_remote_player_meshes_on_texture_debug_change,
};
use crate::entity_anim_spawn::*;
use crate::item_textures::{ItemSpriteMesh, ItemTextureCache};
use crate::model::{
    BIPED_BODY, BIPED_HEAD, BIPED_LEFT_ARM, BIPED_LEFT_LEG, BIPED_MODEL_TEX32, BIPED_MODEL_TEX64,
    BIPED_RIGHT_ARM, BIPED_RIGHT_LEG, COW_MODEL_TEX32, CREEPER_MODEL_TEX64, EntityTextureCache,
    EntityTexturePath, PIG_MODEL_TEX32, QUADRUPED_BODY, QUADRUPED_HEAD, QUADRUPED_LEG_BACK_LEFT,
    QUADRUPED_LEG_BACK_RIGHT, QUADRUPED_LEG_FRONT_LEFT, QUADRUPED_LEG_FRONT_RIGHT,
    SHEEP_MODEL_TEX32, SHEEP_WOOL_MODEL_TEX32, part_mesh, spawn_model,
};
use crate::player_mesh::*;
use crate::specs::{
    BipedModelKind, DROPPED_ITEM_RENDER_SCALE, DROPPED_ITEM_RENDER_Y_OFFSET, QuadrupedModelKind,
    VisualMesh, kind_label, mob_biped_model_kind, mob_model_scale, mob_quadruped_anim_tuning,
    mob_quadruped_model_kind, mob_texture_path, mob_uses_biped_model, mob_uses_entity_model,
    mob_uses_quadruped_model, visual_spec,
};
use rs_render::RenderDebugSettings;
use rs_render::{LookAngles, Player};
use rs_sim::{CameraPerspectiveMode, CameraPerspectiveState, FreecamState, LocalArmSwing};
use rs_ui::ConnectUiState;

pub(crate) use components::{entity_root_translation, player_shadow_emissive_strength};
pub use components::*;
pub use first_person::*;
pub use local_player::*;
pub use motion::*;
pub use registry::*;
pub use remote_apply::*;
pub use skins::*;

const SHEEP_WOOL_TEXTURE_PATH: &str = "entity/sheep/sheep_fur.png";
