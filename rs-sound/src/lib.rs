use bevy::audio::{DefaultSpatialScale, SpatialScale};
use bevy::prelude::*;
use rs_utils::{SoundEventQueue, SoundSettings};

mod events;
mod mappings;
mod runtime;

pub use events::{PlayingSound, emit_entity_sound, emit_ui_sound, emit_world_sound};
pub use mappings::{
    auxiliary_effect_to_sound, block_dig_sound, block_step_sound, button_press_sound_id,
};

pub(crate) const DEFAULT_SOUND_EVENT: &str = "minecraft:gui.button.press";
pub(crate) const MIN_PITCH: f32 = 0.5;
pub(crate) const MAX_PITCH: f32 = 2.0;
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundSettings>()
            .init_resource::<SoundEventQueue>()
            .insert_resource(DefaultSpatialScale(SpatialScale::new(0.2)))
            .add_systems(Startup, runtime::setup_sound_runtime)
            .add_systems(
                Update,
                (
                    runtime::ensure_spatial_listener,
                    runtime::drain_sound_events,
                    runtime::sync_playing_sound_volumes,
                ),
            );
    }
}
