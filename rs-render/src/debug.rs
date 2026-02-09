use bevy::prelude::*;

use crate::components::{ChunkRoot, Player, ShadowCasterLight};

#[derive(Resource, Debug, Clone)]
pub struct RenderDebugSettings {
    pub shadows_enabled: bool,
    pub render_distance_chunks: i32,
}

impl Default for RenderDebugSettings {
    fn default() -> Self {
        Self {
            shadows_enabled: true,
            render_distance_chunks: 12,
        }
    }
}

pub fn apply_render_debug_settings(
    settings: Res<RenderDebugSettings>,
    mut lights: Query<(&mut DirectionalLight, Option<&ShadowCasterLight>)>,
    player: Query<&Transform, With<Player>>,
    mut chunks: Query<(&ChunkRoot, &mut Visibility)>,
) {
    if settings.is_changed() {
        for (mut light, is_shadow) in &mut lights {
            if is_shadow.is_some() {
                light.shadows_enabled = settings.shadows_enabled;
            }
        }
    }

    let Ok(player_transform) = player.get_single() else {
        return;
    };
    let player_chunk_x = (player_transform.translation.x / 16.0).floor() as i32;
    let player_chunk_z = (player_transform.translation.z / 16.0).floor() as i32;
    let max_dist = settings.render_distance_chunks.max(1);

    for (chunk, mut visibility) in &mut chunks {
        let dx = (chunk.key.0 - player_chunk_x).abs();
        let dz = (chunk.key.1 - player_chunk_z).abs();
        let visible = dx <= max_dist && dz <= max_dist;
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
