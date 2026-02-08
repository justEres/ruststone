use bevy::prelude::*;

use crate::components::{LookAngles, Player, PlayerCamera, Velocity};
use crate::input::PlayerInput;

#[derive(Resource)]
pub struct MovementSettings {
    pub walk_speed: f32,
    pub sprint_speed: f32,
    pub mouse_sensitivity: f32,
}

impl Default for MovementSettings {
    fn default() -> Self {
        Self {
            walk_speed: 6.0,
            sprint_speed: 10.0,
            mouse_sensitivity: 0.002,
        }
    }
}

pub fn apply_player_look(
    settings: Res<MovementSettings>,
    input: Res<PlayerInput>,
    mut player_query: Query<(&mut Transform, &mut LookAngles), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    if input.look_delta == Vec2::ZERO {
        return;
    }

    let Some((mut player_transform, mut look)) = player_query.iter_mut().next() else {
        return;
    };
    let Some(mut camera_transform) = camera_query.iter_mut().next() else {
        return;
    };

    look.yaw -= input.look_delta.x * settings.mouse_sensitivity;
    look.pitch -= input.look_delta.y * settings.mouse_sensitivity;
    look.pitch = look.pitch.clamp(-1.54, 1.54);

    player_transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
    camera_transform.rotation = Quat::from_axis_angle(Vec3::X, look.pitch);
}

pub fn apply_player_movement(
    time: Res<Time>,
    settings: Res<MovementSettings>,
    input: Res<PlayerInput>,
    mut query: Query<(&mut Transform, &mut Velocity), With<Player>>,
) {
    let Some((mut transform, mut velocity)) = query.iter_mut().next() else {
        return;
    };

    let right = transform.right();
    let forward = transform.forward();
    let mut direction = right * input.move_axis.x
        + forward * input.move_axis.z
        + Vec3::Y * input.move_axis.y;

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
    }

    let speed = if input.wants_sprint {
        settings.sprint_speed
    } else {
        settings.walk_speed
    };

    velocity.0 = direction * speed;
    transform.translation += velocity.0 * time.delta_secs();
}
