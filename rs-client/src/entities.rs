use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use rs_render::PlayerCamera;
use rs_utils::{AppState, ApplicationState, NetEntityKind, NetEntityMessage};

const REMOTE_PLAYER_HALF_HEIGHT: f32 = 0.9;
const REMOTE_NAME_Y_OFFSET: f32 = 1.35;

#[derive(Default, Resource)]
pub struct RemoteEntityEventQueue {
    events: VecDeque<NetEntityMessage>,
}

impl RemoteEntityEventQueue {
    pub fn push(&mut self, event: NetEntityMessage) {
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> std::collections::vec_deque::Drain<'_, NetEntityMessage> {
        self.events.drain(..)
    }
}

#[derive(Default, Resource)]
pub struct RemoteEntityRegistry {
    pub local_entity_id: Option<i32>,
    pub by_server_id: HashMap<i32, Entity>,
    pub player_entity_by_uuid: HashMap<rs_protocol::protocol::UUID, i32>,
    pub player_name_by_uuid: HashMap<rs_protocol::protocol::UUID, String>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteEntity {
    pub server_id: i32,
    pub kind: NetEntityKind,
    pub on_ground: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct RemoteEntityLook {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityUuid(pub rs_protocol::protocol::UUID);

#[derive(Component, Debug, Clone)]
pub struct RemoteEntityName(pub String);

#[derive(Component)]
pub struct RemotePlayer;

pub fn remote_entity_connection_sync(
    app_state: Res<AppState>,
    mut queue: ResMut<RemoteEntityEventQueue>,
    mut registry: ResMut<RemoteEntityRegistry>,
    mut was_connected: Local<bool>,
) {
    let connected = matches!(app_state.0, ApplicationState::Connected);
    if connected == *was_connected {
        return;
    }
    *was_connected = connected;

    if !registry.by_server_id.is_empty() {
        queue.push(NetEntityMessage::Destroy {
            entity_ids: registry.by_server_id.keys().copied().collect(),
        });
    }
    registry.local_entity_id = None;
    registry.player_entity_by_uuid.clear();
    registry.player_name_by_uuid.clear();
}

pub fn apply_remote_entity_events(
    mut commands: Commands,
    mut queue: ResMut<RemoteEntityEventQueue>,
    mut registry: ResMut<RemoteEntityRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut transform_query: Query<&mut Transform>,
    mut entity_query: Query<(&mut RemoteEntity, &mut RemoteEntityLook)>,
    mut name_query: Query<&mut RemoteEntityName>,
) {
    for event in queue.drain() {
        match event {
            NetEntityMessage::LocalPlayerId { entity_id } => {
                registry.local_entity_id = Some(entity_id);
                if let Some(entity) = registry.by_server_id.remove(&entity_id) {
                    commands.entity(entity).despawn_recursive();
                    registry
                        .player_entity_by_uuid
                        .retain(|_, id| *id != entity_id);
                }
            }
            NetEntityMessage::PlayerInfoAdd { uuid, name } => {
                registry
                    .player_name_by_uuid
                    .insert(uuid.clone(), name.clone());
                if let Some(server_id) = registry.player_entity_by_uuid.get(&uuid).copied()
                    && let Some(entity) = registry.by_server_id.get(&server_id).copied()
                    && let Ok(mut entity_name) = name_query.get_mut(entity)
                {
                    entity_name.0 = name;
                }
            }
            NetEntityMessage::PlayerInfoRemove { uuid } => {
                registry.player_name_by_uuid.remove(&uuid);
            }
            NetEntityMessage::Spawn {
                entity_id,
                uuid,
                kind,
                pos,
                yaw,
                pitch,
                on_ground,
            } => {
                if registry.local_entity_id == Some(entity_id) {
                    continue;
                }

                let display_name = uuid
                    .as_ref()
                    .and_then(|id| registry.player_name_by_uuid.get(id))
                    .cloned()
                    .unwrap_or_else(|| format!("Player {}", entity_id));

                if let Some(existing) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut transform) = transform_query.get_mut(existing) {
                        transform.translation = pos + Vec3::Y * REMOTE_PLAYER_HALF_HEIGHT;
                        transform.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
                    }
                    if let Ok((mut remote_entity, mut look)) = entity_query.get_mut(existing) {
                        remote_entity.kind = kind;
                        remote_entity.on_ground = on_ground.unwrap_or(remote_entity.on_ground);
                        look.yaw = yaw;
                        look.pitch = pitch;
                    }
                    if let Ok(mut name_comp) = name_query.get_mut(existing) {
                        name_comp.0 = display_name;
                    }
                    continue;
                }

                let mesh = meshes.add(Capsule3d::default());
                let material = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.85, 0.78, 0.72),
                    perceptual_roughness: 0.95,
                    metallic: 0.0,
                    ..Default::default()
                });
                let mut spawn_cmd = commands.spawn((
                    Name::new(format!("RemoteEntity[{entity_id}]")),
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform {
                        translation: pos + Vec3::Y * REMOTE_PLAYER_HALF_HEIGHT,
                        rotation: Quat::from_axis_angle(Vec3::Y, yaw),
                        scale: Vec3::new(0.55, 0.9, 0.55),
                    },
                    GlobalTransform::default(),
                    Visibility::Visible,
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                    RemoteEntity {
                        server_id: entity_id,
                        kind,
                        on_ground: on_ground.unwrap_or(false),
                    },
                    RemoteEntityLook { yaw, pitch },
                    RemoteEntityName(display_name),
                ));
                if kind == NetEntityKind::Player {
                    spawn_cmd.insert(RemotePlayer);
                }
                if let Some(uuid) = uuid {
                    registry
                        .player_entity_by_uuid
                        .insert(uuid.clone(), entity_id);
                    spawn_cmd.insert(RemoteEntityUuid(uuid));
                }

                registry.by_server_id.insert(entity_id, spawn_cmd.id());
            }
            NetEntityMessage::MoveDelta {
                entity_id,
                delta,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut transform) = transform_query.get_mut(entity) {
                        transform.translation += delta;
                    }
                    if let Ok((mut remote_entity, _)) = entity_query.get_mut(entity)
                        && let Some(on_ground) = on_ground
                    {
                        remote_entity.on_ground = on_ground;
                    }
                }
            }
            NetEntityMessage::Look {
                entity_id,
                yaw,
                pitch,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut transform) = transform_query.get_mut(entity) {
                        transform.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
                    }
                    if let Ok((mut remote_entity, mut look)) = entity_query.get_mut(entity) {
                        look.yaw = yaw;
                        look.pitch = pitch;
                        if let Some(on_ground) = on_ground {
                            remote_entity.on_ground = on_ground;
                        }
                    }
                }
            }
            NetEntityMessage::Teleport {
                entity_id,
                pos,
                yaw,
                pitch,
                on_ground,
            } => {
                if let Some(entity) = registry.by_server_id.get(&entity_id).copied() {
                    if let Ok(mut transform) = transform_query.get_mut(entity) {
                        transform.translation = pos + Vec3::Y * REMOTE_PLAYER_HALF_HEIGHT;
                        transform.rotation = Quat::from_axis_angle(Vec3::Y, yaw);
                    }
                    if let Ok((mut remote_entity, mut look)) = entity_query.get_mut(entity) {
                        look.yaw = yaw;
                        look.pitch = pitch;
                        remote_entity.on_ground = on_ground.unwrap_or(remote_entity.on_ground);
                    }
                }
            }
            NetEntityMessage::Velocity { .. } => {}
            NetEntityMessage::Destroy { entity_ids } => {
                for entity_id in entity_ids {
                    if let Some(entity) = registry.by_server_id.remove(&entity_id) {
                        commands.entity(entity).despawn_recursive();
                    }
                    registry
                        .player_entity_by_uuid
                        .retain(|_, id| *id != entity_id);
                }
            }
        }
    }
}

pub fn draw_remote_player_names(
    mut contexts: EguiContexts,
    camera_query: Query<(&Camera, &GlobalTransform), With<PlayerCamera>>,
    names_query: Query<(&GlobalTransform, &RemoteEntityName), With<RemotePlayer>>,
) {
    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };
    let ctx = contexts.ctx_mut().unwrap();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("remote_player_names"),
    ));

    for (transform, name) in &names_query {
        let world_pos = transform.translation() + Vec3::Y * REMOTE_NAME_Y_OFFSET;
        let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) else {
            continue;
        };
        let pos = egui::pos2(screen_pos.x, screen_pos.y);
        painter.text(
            pos,
            egui::Align2::CENTER_BOTTOM,
            &name.0,
            egui::TextStyle::Body.resolve(&ctx.style()),
            egui::Color32::WHITE,
        );
    }
}
