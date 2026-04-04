use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::app::Plugin;
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::window::WindowFocused;
use bevy::window::{PresentMode, PrimaryWindow};
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self},
};
use rs_render::{
    AntiAliasingMode, BlockModelResolver, IconQuad, ModelFace, RenderDebugSettings,
    ShadingModel, VanillaBlockShadowMode, default_model_roots,
};
use rs_utils::{
    AppState, ApplicationState, AuthMode, BlockFace, BlockModelKind, BreakIndicator, Chat,
    InventoryItemStack, InventoryState, InventoryWindowInfo, PerfTimings, PlayerStatus,
    ScoreboardState, SoundSettings, TabListHeaderFooter, TitleOverlayState, ToNet, ToNetMessage,
    UiState, WorldTime, block_model_kind, block_registry_key, block_texture_name,
    item_max_durability, item_name, item_registry_key, item_texture_candidates,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

mod connect;
mod debug_items;
mod hud;
mod inventory_interaction;
mod inventory_ui;
mod item_icons;
mod options_persistence;
mod options_ui;
mod overlays;
mod state;
mod tooltips;

pub use connect::UiPlugin;
pub use item_icons::ItemIconCache;
pub use options_persistence::{
    apply_options, load_client_options, load_prism_accounts, save_client_options,
};
pub use state::{ChatAutocompleteState, ConnectUiState, InventoryDragUiState, UiAuthAccount};

pub(crate) const INVENTORY_SLOT_SIZE: f32 = 40.0;
pub(crate) const INVENTORY_SLOT_SPACING: f32 = 4.0;
pub(crate) const DEFAULT_OPTIONS_PATH: &str = "ruststone_options.toml";
pub(crate) const DEBUG_ITEM_CELL: f32 = 52.0;
