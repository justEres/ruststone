use bevy::prelude::*;

mod camera;
mod chunk;
mod components;
mod input;
mod movement;
mod world;

pub use chunk::ChunkUpdateQueue;
pub use components::{LookAngles, Player, PlayerCamera, Velocity, WorldRoot};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<input::PlayerInput>()
            .init_resource::<movement::MovementSettings>()
            .init_resource::<world::WorldSettings>()
            .init_resource::<chunk::ChunkUpdateQueue>()
            .init_resource::<chunk::ChunkRenderState>()
            .init_resource::<chunk::ChunkStore>()
            .init_resource::<chunk::ChunkRenderAssets>()
            .add_systems(Startup, (world::setup_world, camera::spawn_player))
            .add_systems(
                Update,
                (
                    input::collect_player_input,
                    movement::apply_player_look,
                    movement::apply_player_movement,
                )
                    .chain(),
            )
            .add_systems(Update, input::apply_cursor_lock)
            .add_systems(PostUpdate, chunk::apply_chunk_updates);
    }
}
