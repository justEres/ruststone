use bevy::prelude::*;
use bevy::time::Fixed;
use std::sync::Mutex;
use bevy_egui::EguiPrimaryContextPass;

use rs_ui::ConnectUiState;
use rs_utils::{
    AppState, ApplicationState, BreakIndicator, Chat, FromNet, InventoryState, PerfTimings,
    PlayerStatus, ToNet, UiState,
};

use crate::entities;
use crate::entity_model;
use crate::inventory_systems;
use crate::item_textures;
use crate::message_handler;
use crate::net;
use crate::sim;
use crate::sim_systems;

pub struct ClientCorePlugin {
    pub initial_state: ApplicationState,
    to_net: Mutex<Option<ToNet>>,
    from_net: Mutex<Option<FromNet>>,
    connect_ui: Mutex<Option<ConnectUiState>>,
}

impl ClientCorePlugin {
    pub fn new(
        initial_state: ApplicationState,
        to_net: ToNet,
        from_net: FromNet,
        connect_ui: ConnectUiState,
    ) -> Self {
        Self {
            initial_state,
            to_net: Mutex::new(Some(to_net)),
            from_net: Mutex::new(Some(from_net)),
            connect_ui: Mutex::new(Some(connect_ui)),
        }
    }
}

impl Plugin for ClientCorePlugin {
    fn build(&self, app: &mut App) {
        let connect_ui = self
            .connect_ui
            .lock()
            .expect("ConnectUiState lock poisoned")
            .take()
            .expect("ConnectUiState already consumed");
        let to_net = self
            .to_net
            .lock()
            .expect("ToNet lock poisoned")
            .take()
            .expect("ToNet already consumed");
        let from_net = self
            .from_net
            .lock()
            .expect("FromNet lock poisoned")
            .take()
            .expect("FromNet already consumed");

        app.insert_resource(connect_ui)
            .insert_resource(to_net)
            .insert_resource(from_net)
            .insert_resource(AppState(self.initial_state.clone()))
            .insert_resource(Chat::default())
            .insert_resource(UiState::default())
            .insert_resource(InventoryState::default())
            .insert_resource(PlayerStatus::default())
            .insert_resource(BreakIndicator::default())
            .insert_resource(PerfTimings::default())
            .insert_resource(net::events::NetEventQueue::default())
            .insert_resource(entities::RemoteEntityEventQueue::default())
            .insert_resource(entities::RemoteEntityRegistry::default())
            .insert_resource(entities::RemoteSkinDownloader::default())
            .insert_resource(entities::PlayerTextureDebugSettings::default())
            .insert_resource(sim::SimClock::default())
            .insert_resource(sim::CurrentInput::default())
            .insert_resource(sim::SimState::default())
            .insert_resource(sim::SimRenderState::default())
            .insert_resource(sim::VisualCorrectionOffset::default())
            .insert_resource(sim::DebugStats::default())
            .insert_resource(sim::SimReady::default())
            .insert_resource(sim::CorrectionLoopGuard::default())
            .insert_resource(sim::MovementPacketState::default())
            .insert_resource(sim::DebugUiState::default())
            .insert_resource(sim::ZoomState::default())
            .insert_resource(sim::CameraPerspectiveState::default())
            .insert_resource(sim::CameraPerspectiveAltHold::default())
            .insert_resource(sim::LocalArmSwing::default())
            .insert_resource(sim::collision::WorldCollisionMap::default())
            .insert_resource(sim_systems::PredictionHistory::default())
            .insert_resource(sim_systems::LatencyEstimate::default())
            .insert_resource(sim_systems::ActionState::default())
            .insert_resource(sim_systems::FrameTimingState::default())
            .insert_resource(sim_systems::EntityHitboxDebug::default())
            .insert_resource(item_textures::ItemTextureCache::default())
            .insert_resource(entity_model::EntityTextureCache::default());
    }
}

pub struct ClientNetPlugin;

impl Plugin for ClientNetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, message_handler::handle_messages);
    }
}

pub struct ClientInventoryPlugin;

impl Plugin for ClientInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                inventory_systems::hotbar_input_system,
                inventory_systems::inventory_transaction_ack_system,
            ),
        );
    }
}

pub struct ClientItemTexturePlugin;

impl Plugin for ClientItemTexturePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, item_textures::init_item_sprite_mesh)
            .add_systems(Update, item_textures::item_texture_cache_tick);
    }
}

pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                entities::remote_entity_connection_sync.after(message_handler::handle_messages),
                entities::apply_remote_entity_events.after(entities::remote_entity_connection_sync),
                entities::remote_skin_download_tick.after(entities::apply_remote_entity_events),
                entities::apply_remote_player_skins.after(entities::remote_skin_download_tick),
                entities::rebuild_remote_player_meshes_on_texture_debug_change
                    .after(entities::apply_remote_player_skins),
                entities::smooth_remote_entity_motion
                    .after(entities::apply_remote_entity_events)
                    .after(entities::rebuild_remote_player_meshes_on_texture_debug_change),
                entities::animate_remote_player_models.after(entities::smooth_remote_entity_motion),
                entities::animate_remote_biped_models.after(entities::smooth_remote_entity_motion),
                entities::billboard_item_sprites.after(entities::smooth_remote_entity_motion),
            ),
        )
        .add_systems(
            Update,
            (
                entities::apply_held_item_visibility_system,
                entities::apply_item_sprite_textures_system
                    .after(item_textures::item_texture_cache_tick),
                entities::apply_entity_model_textures_system
                    .after(entity_model::entity_texture_cache_tick),
            ),
        )
        .add_systems(
            Update,
            (
                entities::spawn_local_player_model_system
                    .after(sim_systems::apply_visual_transform_system),
                entities::apply_local_player_model_visibility_system
                    .after(entities::spawn_local_player_model_system),
                entities::update_local_player_skin_system
                    .after(entities::spawn_local_player_model_system),
                entities::sync_local_player_skin_model_system
                    .after(entities::update_local_player_skin_system)
                    .before(entities::animate_local_player_model_system),
                entities::first_person_viewmodel_system
                    .after(entities::sync_local_player_skin_model_system),
                entities::animate_first_person_viewmodel_system
                    .after(entities::first_person_viewmodel_system),
                entities::animate_local_player_model_system
                    .after(sim_systems::apply_visual_transform_system),
            ),
        )
        .add_systems(EguiPrimaryContextPass, entities::draw_remote_entity_names);
    }
}

pub struct ClientSimPlugin;

impl Plugin for ClientSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_seconds(0.05))
            .add_systems(
                Update,
                (
                    sim_systems::debug_toggle_system,
                    sim_systems::camera_perspective_toggle_system,
                    sim_systems::camera_perspective_alt_hold_system,
                    sim_systems::input_collect_system,
                    sim_systems::camera_zoom_system
                        .after(rs_render::debug::apply_render_debug_settings),
                    entity_model::entity_texture_cache_tick,
                    sim_systems::visual_smoothing_system,
                    sim_systems::apply_visual_transform_system,
                ),
            )
            .add_systems(
                Update,
                (
                    sim_systems::local_arm_swing_tick_system,
                    sim_systems::local_held_item_view_system
                        .after(sim_systems::apply_visual_transform_system),
                    sim_systems::draw_entity_hitboxes_system
                        .after(sim_systems::apply_visual_transform_system),
                    sim_systems::draw_chunk_debug_system
                        .after(sim_systems::apply_visual_transform_system),
                    sim_systems::world_interaction_system
                        .after(sim_systems::apply_visual_transform_system),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    sim_systems::net_event_apply_system,
                    sim_systems::fixed_sim_tick_system,
                )
                    .chain(),
            )
            .add_systems(EguiPrimaryContextPass, sim_systems::debug_overlay_system)
            ;
    }
}

pub struct ClientTimingPlugin;

#[cfg(feature = "perf_timing")]
impl Plugin for ClientTimingPlugin {
    fn build(&self, app: &mut App) {
        use bevy::diagnostic::FrameTimeDiagnosticsPlugin;

        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(First, sim_systems::frame_timing_start)
            .add_systems(
                Update,
                sim_systems::update_timing_start.before(sim_systems::debug_toggle_system),
            )
            .add_systems(
                Update,
                sim_systems::update_timing_end.after(sim_systems::apply_visual_transform_system),
            )
            .add_systems(
                PostUpdate,
                sim_systems::post_update_timing_start.before(sim_systems::post_update_timing_end),
            )
            .add_systems(
                PostUpdate,
                sim_systems::post_update_timing_end.after(sim_systems::post_update_timing_start),
            )
            .add_systems(
                FixedUpdate,
                sim_systems::fixed_update_timing_start
                    .before(sim_systems::net_event_apply_system),
            )
            .add_systems(
                FixedUpdate,
                sim_systems::fixed_update_timing_end.after(sim_systems::fixed_sim_tick_system),
            )
            .add_systems(Last, sim_systems::frame_timing_end);
    }
}

#[cfg(not(feature = "perf_timing"))]
impl Plugin for ClientTimingPlugin {
    fn build(&self, _app: &mut App) {}
}
