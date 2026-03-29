use super::*;

pub fn fixed_sim_tick_system(
    mut sim_clock: ResMut<SimClock>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_state: ResMut<SimState>,
    mut input: ResMut<CurrentInput>,
    mut history: ResMut<PredictionHistory>,
    mut action_state: ResMut<ActionState>,
    mut player_status: ResMut<rs_utils::PlayerStatus>,
    mut movement_session: ResMut<MovementSession>,
    collision_map: Res<WorldCollisionMap>,
    app_state: Res<AppState>,
    mut params: FixedSimParams,
) {
    let timer = Timing::start();
    if !matches!(app_state.0, ApplicationState::Connected) || !params.sim_ready.0 {
        movement_session.reset_runtime();
        action_state.jump_was_pressed = false;
        action_state.fly_toggle_timer = 0;
        action_state.sent_sneaking = false;
        action_state.sent_sprinting = false;
        action_state.sim_sprinting = false;
        params.timings.fixed_tick_ms = timer.ms();
        return;
    }
    let world = WorldCollision::with_map(&collision_map);
    let tick = sim_clock.tick;
    let mut input_snapshot = input.0;
    let boosted_flying_speed = effective_flying_speed(
        player_status.flying_speed,
        &params.render_debug,
        &player_status,
    );

    let gamemode_allows_flight = matches!(player_status.gamemode, 1 | 3);
    if !gamemode_allows_flight {
        player_status.can_fly = false;
        player_status.flying = false;
        action_state.fly_toggle_timer = 0;
    }

    let jump_pressed = input_snapshot.jump && !action_state.jump_was_pressed;
    action_state.jump_was_pressed = input_snapshot.jump;

    if action_state.fly_toggle_timer > 0 {
        action_state.fly_toggle_timer = action_state.fly_toggle_timer.saturating_sub(1);
    }
    if player_status.can_fly {
        if jump_pressed {
            if action_state.fly_toggle_timer == 0 {
                action_state.fly_toggle_timer = 7;
            } else {
                player_status.flying = !player_status.flying;
                action_state.fly_toggle_timer = 0;
                let _ = params.to_net.0.send(ToNetMessage::ClientAbilities {
                    flags: client_abilities_flags(&player_status),
                    flying_speed: boosted_flying_speed,
                    walking_speed: player_status.walking_speed,
                });
            }
        }
    } else if player_status.flying {
        player_status.flying = false;
        action_state.fly_toggle_timer = 0;
        let _ = params.to_net.0.send(ToNetMessage::ClientAbilities {
            flags: client_abilities_flags(&player_status),
            flying_speed: boosted_flying_speed,
            walking_speed: player_status.walking_speed,
        });
    }

    input_snapshot.can_fly = player_status.can_fly;
    input_snapshot.flying = player_status.flying;
    input_snapshot.flying_speed = boosted_flying_speed;
    input_snapshot.speed_multiplier = (player_status.walking_speed / 0.1).max(0.0);
    input_snapshot.jump_boost_amplifier = player_status.jump_boost_amplifier;

    let sprint_key_down = input_snapshot.sprint;
    let forward_strong = input_snapshot.forward >= SPRINT_FORWARD_THRESHOLD;
    let sprint_eligible = player_status.food > 6 || player_status.can_fly;
    let mut sprinting_state = action_state.sim_sprinting;
    if !sprinting_state
        && sprint_key_down
        && forward_strong
        && sprint_eligible
        && !input_snapshot.sneak
    {
        sprinting_state = true;
    }
    if sprinting_state
        && (!forward_strong || sim_state.current.collided_horizontally || !sprint_eligible)
    {
        sprinting_state = false;
    }
    input_snapshot.sprint = sprinting_state;
    action_state.sim_sprinting = sprinting_state;

    sim_render.previous = sim_state.current;
    let next_state = if movement_session.consume_physics_hold() {
        let mut state = movement_session.last_authoritative_state;
        state.yaw = input_snapshot.yaw;
        state.pitch = input_snapshot.pitch;
        state.vel = Vec3::ZERO;
        state
    } else {
        simulate_tick(&sim_state.current, &input_snapshot, &world)
    };

    history.0.push(PredictedFrame {
        tick,
        input: input_snapshot,
        state: next_state,
    });

    sim_state.current = next_state;
    if player_status.flying && player_status.gamemode != 3 && sim_state.current.on_ground {
        player_status.flying = false;
        let _ = params.to_net.0.send(ToNetMessage::ClientAbilities {
            flags: client_abilities_flags(&player_status),
            flying_speed: boosted_flying_speed,
            walking_speed: player_status.walking_speed,
        });
    }
    sim_clock.tick = sim_clock.tick.wrapping_add(1);
    input.0.jump = false;

    if matches!(app_state.0, ApplicationState::Connected) {
        let current_sneak = input_snapshot.sneak;
        let current_sprint = effective_sprint(&input_snapshot);

        if movement_session.correction_active() {
            params.timings.fixed_tick_ms = timer.ms();
            return;
        }

        if current_sneak != action_state.sent_sneaking {
            let action_id = if current_sneak { 0 } else { 1 };
            if let Some(entity_id) = params.remote_entities.local_entity_id {
                let _ = params.to_net.0.send(ToNetMessage::PlayerAction {
                    entity_id,
                    action_id,
                });
            }
            action_state.sent_sneaking = current_sneak;
        }
        if current_sprint != action_state.sent_sprinting {
            let action_id = if current_sprint { 3 } else { 4 };
            if let Some(entity_id) = params.remote_entities.local_entity_id {
                let _ = params.to_net.0.send(ToNetMessage::PlayerAction {
                    entity_id,
                    action_id,
                });
            }
            action_state.sent_sprinting = current_sprint;
        }
    }
    params.timings.fixed_tick_ms = timer.ms();
}

fn is_water_state(block_state: u16) -> bool {
    matches!(block_state_id(block_state), 8 | 9)
}

pub fn local_movement_sound_system(
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    sim_state: Res<SimState>,
    sim_render: Res<SimRenderState>,
    collision_map: Res<WorldCollisionMap>,
    mut movement_sound: ResMut<MovementSoundState>,
    mut sound_queue: ResMut<SoundEventQueue>,
) {
    if !gameplay_input_allowed(&app_state, &ui_state, &player_status) {
        movement_sound.accumulated_ground_distance = 0.0;
        movement_sound.accumulated_swim_distance = 0.0;
        movement_sound.was_in_water = false;
        return;
    }

    let previous = sim_render.previous;
    let current = sim_state.current;
    let horizontal_delta = Vec2::new(
        current.pos.x - previous.pos.x,
        current.pos.z - previous.pos.z,
    );
    let horizontal_distance = horizontal_delta.length();
    if horizontal_distance <= f32::EPSILON {
        return;
    }

    let feet_block = collision_map.block_at(
        current.pos.x.floor() as i32,
        current.pos.y.floor() as i32,
        current.pos.z.floor() as i32,
    );
    let in_water = is_water_state(feet_block);

    if in_water && !movement_sound.was_in_water {
        emit_ui_sound(&mut sound_queue, "minecraft:random.splash", 0.5, 1.0);
    }
    movement_sound.was_in_water = in_water;

    if in_water {
        movement_sound.accumulated_ground_distance = 0.0;
        movement_sound.accumulated_swim_distance += horizontal_distance;
        if movement_sound.accumulated_swim_distance >= VANILLA_STEP_TRIGGER_DISTANCE {
            emit_world_sound(
                &mut sound_queue,
                "minecraft:game.player.swim",
                current.pos,
                0.35,
                1.0,
                Some(SoundCategory::Player),
            );
            movement_sound.accumulated_swim_distance = 0.0;
        }
        return;
    }

    movement_sound.accumulated_swim_distance = 0.0;
    if !current.on_ground {
        movement_sound.accumulated_ground_distance = 0.0;
        return;
    }

    movement_sound.accumulated_ground_distance += horizontal_distance;
    if movement_sound.accumulated_ground_distance < VANILLA_STEP_TRIGGER_DISTANCE {
        return;
    }

    let below = Vec3::new(current.pos.x, current.pos.y - 0.2, current.pos.z)
        .floor()
        .as_ivec3();
    let ground_state = collision_map.block_at(below.x, below.y, below.z);
    emit_world_sound(
        &mut sound_queue,
        block_step_sound(block_state_id(ground_state)),
        current.pos,
        0.25,
        1.0,
        Some(SoundCategory::Block),
    );
    movement_sound.accumulated_ground_distance = 0.0;
}

pub fn local_arm_swing_tick_system(time: Res<Time>, mut swing: ResMut<LocalArmSwing>) {
    if swing.progress >= 1.0 {
        swing.progress = 1.0;
        return;
    }
    let dt = time.delta_secs().clamp(0.0, 0.05);
    swing.progress = (swing.progress + dt * 3.6).min(1.0);
}

pub fn visual_smoothing_system(
    time: Res<Time>,
    mut offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    let decay = 0.15f32;
    let factor = (1.0 - decay).powf(time.delta_secs() * 20.0);
    offset.0 *= factor;
    debug.smoothing_offset_len = offset.0.length();
    timings.smoothing_ms = timer.ms();
}
