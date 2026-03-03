use super::*;

pub fn world_interaction_system(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    to_net: Res<ToNet>,
    mut inventory_state: ResMut<InventoryState>,
    sim_state: Res<SimState>,
    mut swing: ResMut<LocalArmSwing>,
    mut break_indicator: ResMut<BreakIndicator>,
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    remote_entities: Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
    mut mining: Local<MiningState>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
    {
        if mining.active {
            let _ = to_net.0.send(ToNetMessage::DigCancel {
                x: mining.target_block.x,
                y: mining.target_block.y,
                z: mining.target_block.z,
                face: mining.face,
            });
            *mining = MiningState::default();
        }
        *break_indicator = BreakIndicator::default();
        return;
    }

    let left_just_pressed = mouse.just_pressed(MouseButton::Left);
    let left_held = mouse.pressed(MouseButton::Left);
    let right_click = mouse.just_pressed(MouseButton::Right);
    if !left_held && !right_click {
        if mining.active {
            let _ = to_net.0.send(ToNetMessage::DigCancel {
                x: mining.target_block.x,
                y: mining.target_block.y,
                z: mining.target_block.z,
                face: mining.face,
            });
            *mining = MiningState::default();
        }
        *break_indicator = BreakIndicator::default();
        return;
    }

    let Ok(camera_transform) = camera_query.get_single() else {
        *break_indicator = BreakIndicator::default();
        return;
    };
    let origin = camera_transform.translation();
    let dir = *camera_transform.forward();
    let is_creative = player_status.gamemode == 1;
    let block_reach = if is_creative {
        CREATIVE_BLOCK_REACH
    } else {
        SURVIVAL_BLOCK_REACH
    };
    let entity_reach = if is_creative {
        CREATIVE_ENTITY_REACH
    } else {
        SURVIVAL_ENTITY_REACH
    };

    let block_hit = raycast_block(&collision_map, origin, dir, block_reach);
    let entity_hit = raycast_remote_entity(&remote_entities, origin, dir, entity_reach);

    if left_just_pressed {
        let _ = to_net.0.send(ToNetMessage::SwingArm);
        swing.progress = 0.0;
    }

    if left_held {
        if let Some(entity) = nearest_entity_hit(entity_hit, block_hit) {
            if left_just_pressed {
                let _ = to_net.0.send(ToNetMessage::UseEntity {
                    target_id: entity.entity_id,
                    action: EntityUseAction::Attack,
                });
            }
            if mining.active {
                let _ = to_net.0.send(ToNetMessage::DigCancel {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
                *mining = MiningState::default();
            }
            *break_indicator = BreakIndicator::default();
        } else if let Some(hit) = block_hit {
            let face = normal_to_face_index(hit.normal);
            let block_id = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let held_item = inventory_state.hotbar_item(inventory_state.selected_hotbar_slot);
            let total_secs = estimate_break_time_secs(block_id, held_item);

            if !mining.active || mining.target_block != hit.block || mining.face != face {
                if mining.active {
                    let _ = to_net.0.send(ToNetMessage::DigCancel {
                        x: mining.target_block.x,
                        y: mining.target_block.y,
                        z: mining.target_block.z,
                        face: mining.face,
                    });
                }
                *mining = MiningState {
                    active: true,
                    target_block: hit.block,
                    face,
                    elapsed_secs: 0.0,
                    total_secs,
                    finish_sent: false,
                };
                let _ = to_net.0.send(ToNetMessage::DigStart {
                    x: hit.block.x,
                    y: hit.block.y,
                    z: hit.block.z,
                    face,
                });
            } else {
                mining.elapsed_secs += time.delta_secs();
            }

            let progress = if mining.total_secs > 0.0 {
                (mining.elapsed_secs / mining.total_secs).clamp(0.0, 1.0)
            } else {
                1.0
            };
            *break_indicator = BreakIndicator {
                active: true,
                progress,
                elapsed_secs: mining.elapsed_secs,
                total_secs: mining.total_secs,
            };

            if !mining.finish_sent && progress >= 1.0 {
                let _ = to_net.0.send(ToNetMessage::DigFinish {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
                mining.finish_sent = true;
            }
        } else {
            if mining.active {
                let _ = to_net.0.send(ToNetMessage::DigCancel {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
            }
            *mining = MiningState::default();
            *break_indicator = BreakIndicator::default();
        }
    } else if mining.active {
        let _ = to_net.0.send(ToNetMessage::DigCancel {
            x: mining.target_block.x,
            y: mining.target_block.y,
            z: mining.target_block.z,
            face: mining.face,
        });
        *mining = MiningState::default();
        *break_indicator = BreakIndicator::default();
    }

    if right_click {
        if let Some(entity) = nearest_entity_hit(entity_hit, block_hit) {
            let _ = to_net.0.send(ToNetMessage::UseEntity {
                target_id: entity.entity_id,
                action: EntityUseAction::Interact,
            });
        } else if let Some(hit) = block_hit {
            let face = normal_to_face_index(hit.normal);
            let target_state = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let target_id = block_state_id(target_state);

            if is_interactable_block(target_id) {
                let _ = to_net.0.send(ToNetMessage::PlaceBlock {
                    x: hit.block.x,
                    y: hit.block.y,
                    z: hit.block.z,
                    face: face as i8,
                    cursor_x: 8,
                    cursor_y: 8,
                    cursor_z: 8,
                });
                return;
            }

            let place_pos = hit.block + hit.normal;
            if placement_intersects_player(place_pos, sim_state.current.pos) {
                return;
            }
            let _ = to_net.0.send(ToNetMessage::PlaceBlock {
                x: hit.block.x,
                y: hit.block.y,
                z: hit.block.z,
                face: face as i8,
                cursor_x: 8,
                cursor_y: 8,
                cursor_z: 8,
            });
            if player_status.gamemode != 1 {
                let _ = inventory_state.consume_selected_hotbar_one();
            }
        } else {
            let held_item = inventory_state.hotbar_item(inventory_state.selected_hotbar_slot);
            let _ = to_net.0.send(ToNetMessage::UseItem { held_item });
        }
    }
}

fn is_interactable_block(block_id: u16) -> bool {
    matches!(
        block_id,
        // Containers
        23 | 54 | 61 | 62 | 84 | 130 | 146 | 154 | 158
            // Utility blocks
            | 58 | 116 | 117 | 118 | 120 | 145
            // Doors, trapdoors, fence gates
            | 64 | 71 | 96 | 107 | 167 | 183 | 184 | 185 | 186 | 187 | 193 | 194 | 195 | 196 | 197
            // Buttons/levers
            | 69 | 77 | 143
            // Note block
            | 25
    )
}

fn placement_intersects_player(block_pos: IVec3, player_feet: Vec3) -> bool {
    const PLAYER_HALF_WIDTH: f32 = 0.3;
    const PLAYER_HEIGHT: f32 = 1.8;
    let player_min = Vec3::new(
        player_feet.x - PLAYER_HALF_WIDTH,
        player_feet.y,
        player_feet.z - PLAYER_HALF_WIDTH,
    );
    let player_max = Vec3::new(
        player_feet.x + PLAYER_HALF_WIDTH,
        player_feet.y + PLAYER_HEIGHT,
        player_feet.z + PLAYER_HALF_WIDTH,
    );

    let block_min = Vec3::new(block_pos.x as f32, block_pos.y as f32, block_pos.z as f32);
    let block_max = block_min + Vec3::ONE;

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

#[derive(Clone, Copy)]
struct RayHit {
    block: IVec3,
    normal: IVec3,
    distance: f32,
}

#[derive(Clone, Copy)]
struct EntityHit {
    entity_id: i32,
    distance: f32,
}

fn estimate_break_time_secs(block_id: u16, held_item: Option<rs_utils::InventoryItemStack>) -> f32 {
    let hardness = block_hardness(block_id);
    if hardness < 0.0 {
        return 9999.0;
    }
    if hardness == 0.0 {
        return 0.05;
    }
    let item_id = held_item.map(|stack| stack.item_id);
    let can_harvest = can_harvest_block(item_id, block_id);
    let speed = destroy_speed(item_id, block_id).max(0.1);
    let damage_per_tick = speed / hardness / if can_harvest { 30.0 } else { 100.0 };
    let ticks = (1.0 / damage_per_tick).ceil().max(1.0);
    (ticks / 20.0).clamp(0.05, 9999.0)
}

fn block_hardness(block_id: u16) -> f32 {
    match block_id {
        0 => 0.0,                                      // air
        1 | 4 => 2.0,                                  // stone, cobble
        2 => 0.6,                                      // grass
        3 => 0.5,                                      // dirt
        5 | 17 | 162 => 2.0,                           // planks/log
        12 => 0.5,                                     // sand
        13 => 0.6,                                     // gravel
        14 | 15 | 16 | 21 | 56 | 73 | 74 | 129 => 3.0, // ores
        18 | 161 => 0.2,                               // leaves
        20 => 0.3,                                     // glass
        24 | 45 => 0.8,                                // sandstone, brick
        49 => 50.0,                                    // obsidian
        50 => 0.0,                                     // torch
        54 => 2.5,                                     // chest
        58 => 2.5,                                     // crafting
        61 | 62 => 3.5,                                // furnace
        79 => 0.5,                                     // ice
        80 => 0.2,                                     // snow block
        81 => 0.4,                                     // cactus
        82 => 0.6,                                     // clay
        87 => 0.4,                                     // netherrack
        88 => 0.5,                                     // soulsand
        89 => 0.3,                                     // glowstone
        95 => 0.3,                                     // stained glass
        98 => 1.5,                                     // stone bricks
        155 => 0.8,                                    // quartz block
        159 => 0.8,                                    // stained hardened clay
        171 => 0.8,                                    // carpet/wool-ish break feel
        172 => 1.25,                                   // hardened clay
        173 => 5.0,                                    // coal block
        174 => 0.5,                                    // packed ice
        _ => 1.0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Pickaxe,
    Shovel,
    Axe,
    Sword,
    Shears,
}

fn tool_kind(item_id: i32) -> Option<ToolKind> {
    match item_id {
        256 | 269 | 273 | 277 | 284 => Some(ToolKind::Shovel),
        257 | 270 | 274 | 278 | 285 => Some(ToolKind::Pickaxe),
        258 | 271 | 275 | 279 | 286 => Some(ToolKind::Axe),
        267 | 268 | 272 | 276 | 283 => Some(ToolKind::Sword),
        359 => Some(ToolKind::Shears),
        _ => None,
    }
}

fn tool_harvest_level(item_id: i32) -> i32 {
    match item_id {
        269 | 270 | 271 | 268 => 0, // wood / gold handled by speed
        273 | 274 | 275 | 272 => 1, // stone
        256..=258 | 267 => 2,       // iron
        277..=279 | 276 => 3,       // diamond
        284..=286 | 283 => 0,       // gold
        _ => -1,
    }
}

fn tool_speed(item_id: i32) -> f32 {
    match item_id {
        269..=271 | 268 => 2.0,
        273..=275 | 272 => 4.0,
        256..=258 | 267 => 6.0,
        277..=279 | 276 => 8.0,
        284..=286 | 283 => 12.0,
        359 => 5.0, // shears
        _ => 1.0,
    }
}

fn block_required_tool(block_id: u16) -> Option<(ToolKind, i32)> {
    match block_id {
        // Pickaxe, mostly stone/ore-like blocks.
        1 | 4 | 14 | 15 | 16 | 21 | 22 | 24 | 41 | 42 | 45 | 48 | 57 | 61 | 62 | 73 | 74 | 79
        | 80 | 98 | 101 | 109 | 112 | 121 | 133 | 152 | 155 | 172 | 173 => {
            Some((ToolKind::Pickaxe, 0))
        }
        // Higher harvest tiers.
        56 | 129 | 130 => Some((ToolKind::Pickaxe, 2)), // diamond/emerald/ender chest
        49 | 116 => Some((ToolKind::Pickaxe, 3)),       // obsidian/enchanting table
        // Shovel blocks.
        2 | 3 | 12 | 13 | 78 | 82 | 88 | 110 => Some((ToolKind::Shovel, 0)),
        // Axe blocks.
        5 | 17 | 47 | 53 | 54 | 58 | 64 | 96 | 107 | 134..=136 | 156 | 162 => {
            Some((ToolKind::Axe, 0))
        }
        _ => None,
    }
}

fn block_tool_bonus_kind(block_id: u16) -> Option<ToolKind> {
    match block_id {
        18 | 31 | 32 | 37 | 38 | 39 | 40 | 59 | 81 | 83 | 106 | 111 | 141 | 142 | 161 | 175 => {
            Some(ToolKind::Shears)
        }
        30 => Some(ToolKind::Sword), // cobweb
        _ => block_required_tool(block_id).map(|(kind, _)| kind),
    }
}

fn can_harvest_block(item_id: Option<i32>, block_id: u16) -> bool {
    let Some((required_kind, required_level)) = block_required_tool(block_id) else {
        return true;
    };
    let Some(item_id) = item_id else {
        return false;
    };
    if tool_kind(item_id) != Some(required_kind) {
        return false;
    }
    tool_harvest_level(item_id) >= required_level
}

fn destroy_speed(item_id: Option<i32>, block_id: u16) -> f32 {
    let Some(item_id) = item_id else {
        return 1.0;
    };
    let Some(kind) = tool_kind(item_id) else {
        return 1.0;
    };
    if block_tool_bonus_kind(block_id) == Some(kind) {
        tool_speed(item_id)
    } else {
        1.0
    }
}

fn nearest_entity_hit(
    entity_hit: Option<EntityHit>,
    block_hit: Option<RayHit>,
) -> Option<EntityHit> {
    match (entity_hit, block_hit) {
        (Some(entity), Some(block)) if entity.distance <= block.distance + 0.12 => Some(entity),
        (Some(_), Some(_)) => None,
        (Some(entity), None) => Some(entity),
        _ => None,
    }
}

fn raycast_block(
    world: &WorldCollisionMap,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<RayHit> {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 {
        return None;
    }

    let mut prev_cell = origin.floor().as_ivec3();
    let step = 0.05f32;
    let mut t = step;
    while t <= max_distance {
        let point = origin + dir * t;
        let cell = point.floor().as_ivec3();
        if cell != prev_cell {
            let block_state = world.block_at(cell.x, cell.y, cell.z);
            let block_id = block_state_id(block_state);
            if block_id != 0 && !matches!(block_id, 8 | 9 | 10 | 11) {
                let normal = prev_cell - cell;
                let normal = IVec3::new(normal.x.signum(), normal.y.signum(), normal.z.signum());
                return Some(RayHit {
                    block: cell,
                    normal,
                    distance: t,
                });
            }
            prev_cell = cell;
        }
        t += step;
    }
    None
}

fn raycast_remote_entity(
    remote_entities: &Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<EntityHit> {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 {
        return None;
    }

    let mut nearest: Option<EntityHit> = None;
    for (transform, remote, visual) in remote_entities.iter() {
        let (half_w, height) = match remote.kind {
            rs_utils::NetEntityKind::Player | rs_utils::NetEntityKind::Mob(_) => (0.34, 1.8),
            rs_utils::NetEntityKind::Item => (0.22, 0.35),
            rs_utils::NetEntityKind::ExperienceOrb => (0.18, 0.28),
            rs_utils::NetEntityKind::Object(_) => (0.28, 0.56),
        };
        // Remote entity transforms are rendered with a y-offset; derive collider base from that.
        let feet = transform.translation() - Vec3::Y * visual.y_offset;
        let min = Vec3::new(feet.x - half_w, feet.y, feet.z - half_w);
        let max = Vec3::new(feet.x + half_w, feet.y + height, feet.z + half_w);
        let Some(distance) = ray_aabb_distance(origin, dir, min, max, max_distance) else {
            continue;
        };

        match nearest {
            Some(current) if current.distance <= distance => {}
            _ => {
                nearest = Some(EntityHit {
                    entity_id: remote.server_id,
                    distance,
                });
            }
        }
    }

    nearest
}

pub fn draw_entity_hitboxes_system(
    mut gizmos: Gizmos,
    settings: Res<EntityHitboxDebug>,
    app_state: Res<AppState>,
    entities: Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
) {
    if !settings.enabled || !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    for (transform, remote, visual) in &entities {
        let (half_w, height) = match remote.kind {
            rs_utils::NetEntityKind::Player | rs_utils::NetEntityKind::Mob(_) => (0.34, 1.8),
            rs_utils::NetEntityKind::Item => (0.22, 0.35),
            rs_utils::NetEntityKind::ExperienceOrb => (0.18, 0.28),
            rs_utils::NetEntityKind::Object(_) => (0.28, 0.56),
        };
        let feet = transform.translation() - Vec3::Y * visual.y_offset;
        let min = Vec3::new(feet.x - half_w, feet.y, feet.z - half_w);
        let max = Vec3::new(feet.x + half_w, feet.y + height, feet.z + half_w);
        draw_aabb_lines(&mut gizmos, min, max, Color::srgba(0.2, 1.0, 0.2, 1.0));
    }
}

pub fn draw_chunk_debug_system(
    mut gizmos: Gizmos,
    render_debug: Res<RenderDebugSettings>,
    app_state: Res<AppState>,
    break_indicator: Res<BreakIndicator>,
    freecam: Res<FreecamState>,
    player_status: Res<rs_utils::PlayerStatus>,
    collision_map: Res<WorldCollisionMap>,
    chunks: Query<(&ChunkRoot, &Visibility)>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    if render_debug.show_chunk_borders {
        let color = Color::srgba(0.25, 0.75, 1.0, 1.0);
        for (chunk, vis) in &chunks {
            if matches!(*vis, Visibility::Hidden) {
                continue;
            }
            let base = Vec3::new(chunk.key.0 as f32 * 16.0, 0.0, chunk.key.1 as f32 * 16.0);
            let min = base;
            let max = base + Vec3::new(16.0, 256.0, 16.0);
            draw_aabb_lines(&mut gizmos, min, max, color);
        }
    }

    if render_debug.show_look_ray {
        let Ok(cam) = camera.get_single() else {
            return;
        };
        let origin = cam.translation();
        let dir = *cam.forward();
        let end = origin + dir * 3.0;
        gizmos.line(origin, end, Color::srgba(1.0, 0.2, 0.2, 1.0));
    }

    if render_debug.show_target_block_outline && !freecam.active {
        let Ok(cam) = camera.get_single() else {
            return;
        };
        let origin = cam.translation();
        let dir = *cam.forward();
        let max_reach = if player_status.gamemode == 1 {
            CREATIVE_BLOCK_REACH
        } else {
            SURVIVAL_BLOCK_REACH
        };
        if let Some(hit) = raycast_block(&collision_map, origin, dir, max_reach) {
            let state = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let world = WorldCollision::with_map(&collision_map);
            let boxes = target_outline_boxes(&world, state, hit.block.x, hit.block.y, hit.block.z);
            let inflate = 0.0025;
            for (mut min, mut max) in boxes.iter().copied() {
                min -= Vec3::splat(inflate);
                max += Vec3::splat(inflate);
                draw_aabb_lines(&mut gizmos, min, max, Color::srgba(0.02, 0.02, 0.02, 1.0));
            }
            if break_indicator.active {
                let p = break_indicator.progress.clamp(0.0, 1.0);
                let crack_color = Color::srgb(0.32 + 0.50 * p, 0.32 - 0.20 * p, 0.32 - 0.20 * p);
                let crack_inflate = 0.008 + p * 0.010;
                for (min, max) in boxes {
                    draw_aabb_lines(
                        &mut gizmos,
                        min - Vec3::splat(crack_inflate),
                        max + Vec3::splat(crack_inflate),
                        crack_color,
                    );
                }
            }
        }
    }
}

fn target_outline_boxes(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
) -> Vec<(Vec3, Vec3)> {
    let mut boxes = debug_block_collision_boxes(world, block_state, block_x, block_y, block_z);
    if !boxes.is_empty() {
        return boxes;
    }

    let min = Vec3::new(block_x as f32, block_y as f32, block_z as f32);
    let max = min + Vec3::ONE;
    let id = block_state_id(block_state);

    let fallback = match block_model_kind(id) {
        rs_utils::BlockModelKind::Cross => {
            let h = if id == 175 { 1.0 } else { 0.875 };
            Some((min + Vec3::new(0.1, 0.0, 0.1), min + Vec3::new(0.9, h, 0.9)))
        }
        rs_utils::BlockModelKind::TorchLike => Some((
            min + Vec3::new(0.4, 0.0, 0.4),
            min + Vec3::new(0.6, 0.75, 0.6),
        )),
        rs_utils::BlockModelKind::Custom => match id {
            26 => Some((min, min + Vec3::new(1.0, 9.0 / 16.0, 1.0))), // bed
            27 | 28 | 66 | 157 | 171 => Some((min, min + Vec3::new(1.0, 1.0 / 16.0, 1.0))), // rails/carpet
            78 => {
                let h = ((block_state_meta(block_state) & 0x7) as f32 + 1.0) / 8.0;
                Some((min, min + Vec3::new(1.0, h.clamp(0.125, 1.0), 1.0)))
            }
            _ => Some((min, max)),
        },
        _ => Some((min, max)),
    };

    if let Some(bb) = fallback {
        boxes.push(bb);
    }
    boxes
}

fn draw_aabb_lines(gizmos: &mut Gizmos, min: Vec3, max: Vec3, color: Color) {
    let p000 = Vec3::new(min.x, min.y, min.z);
    let p001 = Vec3::new(min.x, min.y, max.z);
    let p010 = Vec3::new(min.x, max.y, min.z);
    let p011 = Vec3::new(min.x, max.y, max.z);
    let p100 = Vec3::new(max.x, min.y, min.z);
    let p101 = Vec3::new(max.x, min.y, max.z);
    let p110 = Vec3::new(max.x, max.y, min.z);
    let p111 = Vec3::new(max.x, max.y, max.z);

    gizmos.line(p000, p001, color);
    gizmos.line(p001, p011, color);
    gizmos.line(p011, p010, color);
    gizmos.line(p010, p000, color);

    gizmos.line(p100, p101, color);
    gizmos.line(p101, p111, color);
    gizmos.line(p111, p110, color);
    gizmos.line(p110, p100, color);

    gizmos.line(p000, p100, color);
    gizmos.line(p001, p101, color);
    gizmos.line(p010, p110, color);
    gizmos.line(p011, p111, color);
}

fn ray_aabb_distance(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_distance: f32,
) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = max_distance;

    for axis in 0..3 {
        let (origin_axis, dir_axis, min_axis, max_axis) = match axis {
            0 => (origin.x, dir.x, min.x, max.x),
            1 => (origin.y, dir.y, min.y, max.y),
            _ => (origin.z, dir.z, min.z, max.z),
        };

        if dir_axis.abs() <= f32::EPSILON {
            if origin_axis < min_axis || origin_axis > max_axis {
                return None;
            }
            continue;
        }

        let inv = 1.0 / dir_axis;
        let mut t1 = (min_axis - origin_axis) * inv;
        let mut t2 = (max_axis - origin_axis) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }

        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_max < t_min {
            return None;
        }
    }

    if t_min <= max_distance {
        Some(t_min.max(0.0))
    } else {
        None
    }
}

fn normal_to_face_index(normal: IVec3) -> u8 {
    match (normal.x, normal.y, normal.z) {
        (0, -1, 0) => 0, // bottom
        (0, 1, 0) => 1,  // top
        (0, 0, -1) => 2, // north
        (0, 0, 1) => 3,  // south
        (-1, 0, 0) => 4, // west
        (1, 0, 0) => 5,  // east
        _ => 1,
    }
}

fn yaw_deg_to_cardinal(yaw_deg_mc: f32) -> (&'static str, &'static str) {
    // Minecraft 1.8 yaw: 0 = South (+Z), 90 = West (-X), 180 = North (-Z), 270 = East (+X).
    let yaw = yaw_deg_mc.rem_euclid(360.0);
    let idx = ((yaw / 90.0).round() as i32).rem_euclid(4);
    match idx {
        0 => ("South", "+Z"),
        1 => ("West", "-X"),
        2 => ("North", "-Z"),
        _ => ("East", "+X"),
    }
}

fn block_model_kind_label(kind: rs_utils::BlockModelKind) -> &'static str {
    match kind {
        rs_utils::BlockModelKind::FullCube => "FullCube",
        rs_utils::BlockModelKind::Cross => "Cross",
        rs_utils::BlockModelKind::Slab => "Slab",
        rs_utils::BlockModelKind::Stairs => "Stairs",
        rs_utils::BlockModelKind::Fence => "Fence",
        rs_utils::BlockModelKind::Pane => "Pane",
        rs_utils::BlockModelKind::Fluid => "Fluid",
        rs_utils::BlockModelKind::TorchLike => "TorchLike",
        rs_utils::BlockModelKind::Custom => "Custom",
    }
}

pub fn debug_overlay_system(
    mut contexts: EguiContexts,
    debug: Res<DebugStats>,
    sim_clock: Res<SimClock>,
    history: Res<PredictionHistory>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut debug_ui: ResMut<DebugUiState>,
    mut render_debug: ResMut<RenderDebugSettings>,
    render_perf: Res<RenderPerfStats>,
    sim_state: Res<SimState>,
    input: Res<CurrentInput>,
    player_status: Res<rs_utils::PlayerStatus>,
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    if !debug_ui.open {
        timings.debug_ui_ms = 0.0;
        return;
    }
    let ctx = contexts.ctx_mut().unwrap();
    egui::Window::new("Debug")
        .default_pos(egui::pos2(12.0, 12.0))
        .show(ctx, |ui| {
            let frame_ms = (time.delta_secs_f64() * 1000.0) as f32;
            let performance_section = egui::CollapsingHeader::new("Performance")
                .default_open(debug_ui.show_performance)
                .show(ui, |ui| {
                    ui.separator();
                    if let Some(fps) = diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|d| d.smoothed())
                    {
                        ui.label(format!("fps: {:.1}", fps));
                    } else {
                        ui.label("fps: n/a");
                    }
                    ui.label(format!("frame ms (delta): {:.2}", frame_ms));
                    ui.label(format!(
                        "main thread ms: {:.2} {}",
                        timings.main_thread_ms,
                        if frame_ms > 0.0 {
                            format!("{:.1}%", (timings.main_thread_ms / frame_ms) * 100.0)
                        } else {
                            "n/a".to_string()
                        }
                    ));
                });
            debug_ui.show_performance = performance_section.fully_open();

            let render_section = egui::CollapsingHeader::new("Render")
                .default_open(debug_ui.show_render)
                .show(ui, |ui| {
                    ui.separator();
                    let layers_section = egui::CollapsingHeader::new("Layers")
                        .default_open(debug_ui.render_show_layers)
                        .show(ui, |ui| {
                            ui.separator();
                            ui.checkbox(&mut render_debug.show_layer_entities, "Layer: entities");
                            ui.checkbox(
                                &mut render_debug.show_layer_chunks_opaque,
                                "Layer: chunks opaque",
                            );
                            ui.checkbox(
                                &mut render_debug.show_layer_chunks_cutout,
                                "Layer: chunks cutout",
                            );
                            ui.checkbox(
                                &mut render_debug.show_layer_chunks_transparent,
                                "Layer: chunks transparent",
                            );
                        });
                    debug_ui.render_show_layers = layers_section.fully_open();
                    ui.separator();
                    ui.checkbox(&mut render_debug.show_coordinates, "Coordinates");
                    ui.checkbox(&mut render_debug.show_look_info, "Look info");
                    ui.checkbox(&mut render_debug.show_look_ray, "Look ray");
                    ui.checkbox(
                        &mut render_debug.show_target_block_outline,
                        "Target block outline",
                    );

                    if render_debug.show_coordinates || render_debug.show_look_info {
                        ui.separator();
                    }
                    if render_debug.show_coordinates {
                        let pos = sim_state.current.pos;
                        let block = pos.floor().as_ivec3();
                        let chunk_x = block.x.div_euclid(16);
                        let chunk_z = block.z.div_euclid(16);
                        ui.label(format!("pos: {:.3} {:.3} {:.3}", pos.x, pos.y, pos.z));
                        ui.label(format!("block: {} {} {}", block.x, block.y, block.z));
                        ui.label(format!("chunk: {} {}", chunk_x, chunk_z));
                    }
                    if render_debug.show_look_info {
                        let yaw_mc = (std::f32::consts::PI - input.0.yaw).to_degrees();
                        let pitch_mc = (-input.0.pitch).to_degrees();
                        let (card, axis) = yaw_deg_to_cardinal(yaw_mc);
                        ui.label(format!("yaw/pitch: {:.1} / {:.1}", yaw_mc, pitch_mc));
                        ui.label(format!("facing: {} ({})", card, axis));

                        if let Ok(camera_transform) = camera_query.get_single() {
                            let origin = camera_transform.translation();
                            let dir = *camera_transform.forward();
                            let max_reach = if player_status.gamemode == 1 {
                                CREATIVE_BLOCK_REACH
                            } else {
                                SURVIVAL_BLOCK_REACH
                            };
                            if let Some(hit) = raycast_block(&collision_map, origin, dir, max_reach)
                            {
                                let state =
                                    collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
                                let id = block_state_id(state);
                                let meta = block_state_meta(state);
                                let kind = block_model_kind(id);
                                let reg = block_registry_key(id).unwrap_or("minecraft:unknown");
                                let world = WorldCollision::with_map(&collision_map);
                                let boxes = debug_block_collision_boxes(
                                    &world,
                                    state,
                                    hit.block.x,
                                    hit.block.y,
                                    hit.block.z,
                                );

                                ui.label(format!(
                                    "target block: {} {} {}",
                                    hit.block.x, hit.block.y, hit.block.z
                                ));
                                ui.label(format!(
                                    "id/state/meta: {} / {} / {}  kind: {}",
                                    id,
                                    state,
                                    meta,
                                    block_model_kind_label(kind)
                                ));
                                ui.label(format!("registry: {}", reg));
                                ui.label(format!("collision boxes: {}", boxes.len()));
                                for (idx, (min, max)) in boxes.iter().take(4).enumerate() {
                                    let size = *max - *min;
                                    ui.label(format!(
                                        "box{} min({:.3},{:.3},{:.3}) size({:.3},{:.3},{:.3})",
                                        idx, min.x, min.y, min.z, size.x, size.y, size.z
                                    ));
                                }
                                if boxes.len() > 4 {
                                    ui.label(format!("... {} more boxes", boxes.len() - 4));
                                }
                            } else {
                                ui.label("target block: none");
                            }
                        }
                    }
                });
            debug_ui.show_render = render_section.fully_open();

            let prediction_section = egui::CollapsingHeader::new("Prediction")
                .default_open(debug_ui.show_prediction)
                .show(ui, |ui| {
                    ui.separator();
                    ui.label(format!("tick: {}", sim_clock.tick));
                    ui.label(format!("history cap: {}", history.0.capacity()));
                    ui.label(format!("last correction: {:.4}", debug.last_correction));
                    ui.label(format!("last replay ticks: {}", debug.last_replay));
                    ui.label(format!(
                        "smoothing offset: {:.4}",
                        debug.smoothing_offset_len
                    ));
                    ui.label(format!("one-way ticks: {}", debug.one_way_ticks));
                });
            debug_ui.show_prediction = prediction_section.fully_open();

            if debug_ui.show_performance {
                ui.separator();
                let schedule_section = egui::CollapsingHeader::new("Schedule Timings")
                    .default_open(debug_ui.perf_show_schedules)
                    .show(ui, |_ui| {});
                debug_ui.perf_show_schedules = schedule_section.fully_open();
                let render_stats_section = egui::CollapsingHeader::new("Render Timings")
                    .default_open(debug_ui.perf_show_render_stats)
                    .show(ui, |_ui| {});
                debug_ui.perf_show_render_stats = render_stats_section.fully_open();
                let pct = |ms: f32| {
                    if frame_ms <= 0.0 {
                        None
                    } else {
                        Some((ms / frame_ms).max(0.0) * 100.0)
                    }
                };
                let fmt_pct = |ms: f32| {
                    pct(ms)
                        .map(|p| format!("{:.1}%", p))
                        .unwrap_or_else(|| "n/a".to_string())
                };
                if debug_ui.perf_show_schedules {
                    ui.label(format!(
                        "handle_messages: {:.3}ms {}",
                        timings.handle_messages_ms,
                        fmt_pct(timings.handle_messages_ms)
                    ));
                    ui.label(format!(
                        "update schedule: {:.3}ms {}",
                        timings.update_ms,
                        fmt_pct(timings.update_ms)
                    ));
                    ui.label(format!(
                        "post update: {:.3}ms {}",
                        timings.post_update_ms,
                        fmt_pct(timings.post_update_ms)
                    ));
                    ui.label(format!(
                        "fixed update: {:.3}ms {}",
                        timings.fixed_update_ms,
                        fmt_pct(timings.fixed_update_ms)
                    ));
                    ui.label(format!(
                        "input_collect: {:.3}ms {}",
                        timings.input_collect_ms,
                        fmt_pct(timings.input_collect_ms)
                    ));
                    ui.label(format!(
                        "net_apply: {:.3}ms {}",
                        timings.net_apply_ms,
                        fmt_pct(timings.net_apply_ms)
                    ));
                    ui.label(format!(
                        "fixed_tick: {:.3}ms {}",
                        timings.fixed_tick_ms,
                        fmt_pct(timings.fixed_tick_ms)
                    ));
                    ui.label(format!(
                        "smoothing: {:.3}ms {}",
                        timings.smoothing_ms,
                        fmt_pct(timings.smoothing_ms)
                    ));
                    ui.label(format!(
                        "apply_transform: {:.3}ms {}",
                        timings.apply_transform_ms,
                        fmt_pct(timings.apply_transform_ms)
                    ));
                    ui.label(format!(
                        "debug_ui: {:.3}ms {}",
                        timings.debug_ui_ms,
                        fmt_pct(timings.debug_ui_ms)
                    ));
                    ui.label(format!(
                        "ui: {:.3}ms {}",
                        timings.ui_ms,
                        fmt_pct(timings.ui_ms)
                    ));
                }
                if debug_ui.perf_show_render_stats {
                    ui.separator();
                    ui.label(format!(
                        "mesh build ms: {:.2} (avg {:.2}) [async]",
                        render_perf.last_mesh_build_ms, render_perf.avg_mesh_build_ms
                    ));
                    ui.label(format!(
                        "mesh apply ms: {:.2} (avg {:.2}) {}",
                        render_perf.last_apply_ms,
                        render_perf.avg_apply_ms,
                        fmt_pct(render_perf.last_apply_ms)
                    ));
                    ui.label(format!(
                        "mesh enqueue ms: {:.2} (avg {:.2}) {}",
                        render_perf.last_enqueue_ms,
                        render_perf.avg_enqueue_ms,
                        fmt_pct(render_perf.last_enqueue_ms)
                    ));
                    ui.label(format!(
                        "occlusion cull ms: {:.2} {}",
                        render_perf.occlusion_cull_ms,
                        fmt_pct(render_perf.occlusion_cull_ms)
                    ));
                    ui.label(format!(
                        "render debug ms: {:.2} {}",
                        render_perf.apply_debug_ms,
                        fmt_pct(render_perf.apply_debug_ms)
                    ));
                    ui.label(format!(
                        "render stats ms: {:.2} {}",
                        render_perf.gather_stats_ms,
                        fmt_pct(render_perf.gather_stats_ms)
                    ));
                    ui.label(format!(
                        "mesh applied: {} in_flight: {} updates: {} (raw {})",
                        render_perf.last_meshes_applied,
                        render_perf.in_flight,
                        render_perf.last_updates,
                        render_perf.last_updates_raw
                    ));
                    ui.label(format!(
                        "meshes: dist {} / {} view {} / {}",
                        render_perf.visible_meshes_distance,
                        render_perf.total_meshes,
                        render_perf.visible_meshes_view,
                        render_perf.total_meshes
                    ));
                    ui.label(format!(
                        "chunks: {} / {} (distance)",
                        render_perf.visible_chunks, render_perf.total_chunks
                    ));
                    ui.label(format!(
                        "chunks after occlusion: {} (occluded {})",
                        render_perf.visible_chunks_after_occlusion, render_perf.occluded_chunks
                    ));
                    ui.separator();
                    ui.label(format!(
                        "mat pass w: o={:.1} c={:.1} cc={:.1} t={:.1}",
                        render_perf.mat_pass_opaque,
                        render_perf.mat_pass_cutout,
                        render_perf.mat_pass_cutout_culled,
                        render_perf.mat_pass_transparent
                    ));
                    ui.label(format!(
                        "mat alpha: o={} c={} cc={} t={}",
                        render_perf.mat_alpha_opaque,
                        render_perf.mat_alpha_cutout,
                        render_perf.mat_alpha_cutout_culled,
                        render_perf.mat_alpha_transparent
                    ));
                    ui.label(format!(
                        "mat unlit: o={} c={} cc={} t={}",
                        render_perf.mat_unlit_opaque,
                        render_perf.mat_unlit_cutout,
                        render_perf.mat_unlit_cutout_culled,
                        render_perf.mat_unlit_transparent
                    ));
                }
            }
        });

    let elapsed_ms = timer.ms();
    timings.debug_ui_ms = elapsed_ms;
}
