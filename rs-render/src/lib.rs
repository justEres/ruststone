use bevy::prelude::*;

mod camera;
mod components;
mod input;
mod movement;
mod world;

pub use components::{LookAngles, Player, PlayerCamera, Velocity, WorldRoot};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<input::PlayerInput>()
            .init_resource::<movement::MovementSettings>()
            .init_resource::<world::WorldSettings>()
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
            .add_systems(Update, input::apply_cursor_lock);
    }
}
