use super::*;

pub fn debug_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut debug_ui: ResMut<DebugUiState>,
    mut hitbox_debug: ResMut<EntityHitboxDebug>,
    ui_state: Res<UiState>,
) {
    if ui_state.chat_open || ui_state.inventory_open {
        return;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        debug_ui.open = !debug_ui.open;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        hitbox_debug.enabled = !hitbox_debug.enabled;
    }
    if keys.just_pressed(KeyCode::F6) {
        debug_ui.perf_monitor_open = !debug_ui.perf_monitor_open;
    }
    if keys.just_pressed(KeyCode::F7) {
        debug_ui.perf_monitor_compact = !debug_ui.perf_monitor_compact;
    }
}

pub fn camera_perspective_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
) {
    if ui_state.chat_open || ui_state.inventory_open || ui_state.paused {
        return;
    }

    let alt_pressed = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if alt_pressed || !keys.just_pressed(KeyCode::F5) {
        return;
    }

    let cycle = |mode| match mode {
        CameraPerspectiveMode::FirstPerson => CameraPerspectiveMode::ThirdPersonBack,
        CameraPerspectiveMode::ThirdPersonBack => CameraPerspectiveMode::ThirdPersonFront,
        CameraPerspectiveMode::ThirdPersonFront => CameraPerspectiveMode::FirstPerson,
    };
    perspective.mode = cycle(perspective.mode);
    if let Some(saved) = alt_hold.saved_mode {
        alt_hold.saved_mode = Some(cycle(saved));
    }
}

pub fn camera_perspective_alt_hold_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
) {
    if ui_state.chat_open || ui_state.inventory_open || ui_state.paused {
        alt_hold.saved_mode = None;
        return;
    }

    let alt_pressed = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if alt_pressed {
        if alt_hold.saved_mode.is_none() {
            alt_hold.saved_mode = Some(perspective.mode);
        }
        perspective.mode = CameraPerspectiveMode::ThirdPersonBack;
    } else if let Some(saved) = alt_hold.saved_mode.take() {
        perspective.mode = saved;
    }
}

pub fn freecam_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    app_state: Res<AppState>,
    mut freecam: ResMut<FreecamState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
    mut input: ResMut<CurrentInput>,
    mut player_query: Query<(&GlobalTransform, &mut LookAngles), With<Player>>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
) {
    if ui_state.chat_open || ui_state.inventory_open || ui_state.paused {
        return;
    }
    if !matches!(app_state.0, ApplicationState::Connected) || !keys.just_pressed(KeyCode::F4) {
        return;
    }

    let Ok(camera_gt) = camera_query.get_single() else {
        return;
    };
    let camera_forward = camera_gt.forward().as_vec3();
    let (yaw, pitch) = yaw_pitch_from_forward(camera_forward);
    perspective.mode = CameraPerspectiveMode::FirstPerson;
    alt_hold.saved_mode = None;

    if !freecam.active {
        freecam.active = true;
        freecam.position = camera_gt.translation();
        input.0.yaw = yaw;
        input.0.pitch = pitch;
        return;
    }

    freecam.active = false;
    if let Ok((_player_gt, mut look)) = player_query.get_single_mut() {
        look.yaw = yaw;
        look.pitch = pitch;
    }
    input.0.yaw = yaw;
    input.0.pitch = pitch;
}

pub fn freecam_move_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<CurrentInput>,
    mut freecam: ResMut<FreecamState>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    if !freecam.active {
        return;
    }

    let Ok(player_gt) = player_query.get_single() else {
        return;
    };
    let Ok(mut camera_local) = camera_query.get_single_mut() else {
        return;
    };

    let dt = time.delta_secs().clamp(0.0, 0.05);
    let yaw_rot = Quat::from_axis_angle(Vec3::Y, input.0.yaw);
    let pitch_rot = Quat::from_axis_angle(Vec3::X, input.0.pitch);
    let world_rot = yaw_rot * pitch_rot;
    let forward = world_rot * -Vec3::Z;
    let right = world_rot * Vec3::X;

    let mut move_forward = 0.0;
    let mut move_strafe = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        move_forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        move_forward -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        move_strafe += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        move_strafe -= 1.0;
    }
    let vertical = if keys.pressed(KeyCode::Space) {
        1.0
    } else {
        0.0
    } - if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        1.0
    } else {
        0.0
    };

    let mut wish = forward * move_forward + right * move_strafe + Vec3::Y * vertical;
    if wish.length_squared() > 1.0 {
        wish = wish.normalize();
    }
    let sprint = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let speed = if sprint { 26.0 } else { 11.0 };
    freecam.position += wish * speed * dt;

    let player_world = player_gt.compute_transform();
    let inv_player_rot = player_world.rotation.inverse();
    camera_local.translation = inv_player_rot * (freecam.position - player_world.translation);
    camera_local.rotation = inv_player_rot * world_rot;
}

pub fn apply_visual_transform_system(
    fixed_time: Res<Time<Fixed>>,
    input: Res<CurrentInput>,
    perspective: Res<CameraPerspectiveState>,
    sim_render: Res<SimRenderState>,
    sim_state: Res<SimState>,
    offset: Res<VisualCorrectionOffset>,
    collision_map: Res<WorldCollisionMap>,
    freecam: Res<FreecamState>,
    mut player_query: Query<(&mut Transform, &mut LookAngles), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
    mut eye_height: Local<f32>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    const EYE_HEIGHT_STAND: f32 = 1.62;
    const EYE_HEIGHT_SNEAK: f32 = 1.54;
    let alpha = fixed_time.overstep_fraction().clamp(0.0, 1.0);
    if let Ok((mut player_transform, mut look)) = player_query.get_single_mut() {
        let interpolated = sim_render.previous.pos.lerp(sim_state.current.pos, alpha);
        let pos = interpolated + offset.0;
        player_transform.translation = pos;
        if !freecam.active {
            look.yaw = input.0.yaw;
            look.pitch = input.0.pitch;
        }
        player_transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
        if let Ok(mut camera_transform) = camera_query.get_single_mut() {
            if freecam.active {
                timings.apply_transform_ms = timer.ms();
                return;
            }
            let target_eye_height = if input.0.sneak {
                EYE_HEIGHT_SNEAK
            } else {
                EYE_HEIGHT_STAND
            };
            if *eye_height <= 0.0 {
                *eye_height = target_eye_height;
            } else {
                *eye_height += (target_eye_height - *eye_height) * 0.5;
            }

            let pitch_rot = Quat::from_axis_angle(Vec3::X, look.pitch);
            let base_eye_local = Vec3::new(0.0, *eye_height, 0.0);

            let (mut cam_local, cam_local_rot) = match perspective.mode {
                CameraPerspectiveMode::FirstPerson => (base_eye_local, pitch_rot),
                CameraPerspectiveMode::ThirdPersonBack => {
                    let offset_local =
                        pitch_rot * Vec3::new(0.0, 0.0, perspective.third_person_distance);
                    (base_eye_local + offset_local, pitch_rot)
                }
                CameraPerspectiveMode::ThirdPersonFront => {
                    let offset_local =
                        pitch_rot * Vec3::new(0.0, 0.0, -perspective.third_person_distance);
                    (
                        base_eye_local + offset_local,
                        pitch_rot * Quat::from_rotation_y(std::f32::consts::PI),
                    )
                }
            };

            if !matches!(perspective.mode, CameraPerspectiveMode::FirstPerson) {
                let player_rot = player_transform.rotation;
                let anchor_world = player_transform.translation + Vec3::Y * *eye_height;
                let desired_world = player_transform.translation + (player_rot * cam_local);
                let clipped_world =
                    clip_camera_to_world(&collision_map, anchor_world, desired_world);
                cam_local = player_rot.inverse() * (clipped_world - player_transform.translation);
            }

            camera_transform.translation = cam_local;
            camera_transform.rotation = cam_local_rot;
        }
    }
    timings.apply_transform_ms = timer.ms();
}

fn clip_camera_to_world(world: &WorldCollisionMap, anchor: Vec3, desired: Vec3) -> Vec3 {
    let delta = desired - anchor;
    let dist = delta.length();
    if dist <= 0.001 {
        return desired;
    }

    let dir = delta / dist;
    let step = 0.08f32;
    let mut t = step;
    let mut prev_cell = anchor.floor().as_ivec3();
    while t <= dist {
        let point = anchor + dir * t;
        let cell = point.floor().as_ivec3();
        if cell != prev_cell {
            let block_state = world.block_at(cell.x, cell.y, cell.z);
            if crate::sim::collision::is_solid(block_state) {
                return anchor + dir * (t - 0.12).max(0.0);
            }
            prev_cell = cell;
        }
        t += step;
    }

    desired
}
