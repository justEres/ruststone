use super::*;

const MOVE_PKT_GROUND: u8 = 0;
const MOVE_PKT_LOOK: u8 = 1;
const MOVE_PKT_POS: u8 = 2;
const MOVE_PKT_POS_LOOK: u8 = 3;

fn has_loaded_player_chunk(collision_map: &WorldCollisionMap, pos: Vec3) -> bool {
    let chunk_x = (pos.x.floor() as i32).div_euclid(16);
    let chunk_z = (pos.z.floor() as i32).div_euclid(16);
    collision_map.has_chunk(chunk_x, chunk_z)
}

fn record_sent_movement_packet(
    move_pkt_state: &mut MovementPacketState,
    kind: u8,
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
) {
    move_pkt_state.last_sent_initialized = true;
    move_pkt_state.last_sent_kind = kind;
    move_pkt_state.last_sent_pos = pos;
    move_pkt_state.last_sent_yaw_deg = yaw;
    move_pkt_state.last_sent_pitch_deg = pitch;
    move_pkt_state.last_sent_on_ground = on_ground;
}

fn log_outgoing_movement_packet(
    tick: u32,
    source: &'static str,
    kind: u8,
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
) {
    let kind_name = match kind {
        MOVE_PKT_GROUND => "ground",
        MOVE_PKT_LOOK => "look",
        MOVE_PKT_POS => "pos",
        MOVE_PKT_POS_LOOK => "poslook",
        _ => "unknown",
    };
    tracing::info!(
        tick,
        source,
        kind = kind_name,
        x = pos.x,
        y = pos.y,
        z = pos.z,
        yaw,
        pitch,
        on_ground,
        "Outgoing movement packet"
    );
}

fn estimate_server_tick(
    history: &PredictionHistory,
    latest_tick: u32,
    estimated_tick: u32,
    server_state: &crate::sim::PlayerSimState,
) -> (u32, i32) {
    let search_radius = 8u32;
    let start = estimated_tick.saturating_sub(search_radius);
    let end = latest_tick.min(estimated_tick.saturating_add(search_radius));

    let mut best_tick = estimated_tick;
    let mut best_score = f32::INFINITY;
    let mut found = false;

    for tick in start..=end {
        let Some(frame) = history.0.get_by_tick(tick) else {
            continue;
        };
        let predicted = frame.state;
        let pos_err = server_state.pos.distance_squared(predicted.pos);
        let vel_err = server_state.vel.distance_squared(predicted.vel);
        let yaw_err = (server_state.yaw - predicted.yaw).abs();
        let pitch_err = (server_state.pitch - predicted.pitch).abs();
        let ground_err = if server_state.on_ground == predicted.on_ground {
            0.0
        } else {
            0.25
        };
        let score = pos_err + vel_err * 0.35 + yaw_err * 0.02 + pitch_err * 0.02 + ground_err;
        if score < best_score {
            best_score = score;
            best_tick = tick;
            found = true;
        }
    }

    if !found {
        return (estimated_tick, 0);
    }

    (
        best_tick,
        best_tick as i32 - estimated_tick as i32,
    )
}

pub fn fixed_sim_tick_system(
    mut sim_clock: ResMut<SimClock>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_state: ResMut<SimState>,
    mut input: ResMut<CurrentInput>,
    mut history: ResMut<PredictionHistory>,
    mut latency: ResMut<LatencyEstimate>,
    mut action_state: ResMut<ActionState>,
    mut player_status: ResMut<rs_utils::PlayerStatus>,
    mut correction_guard: ResMut<CorrectionLoopGuard>,
    mut move_pkt_state: ResMut<MovementPacketState>,
    collision_map: Res<WorldCollisionMap>,
    app_state: Res<AppState>,
    mut params: FixedSimParams,
) {
    let timer = Timing::start();
    if !matches!(app_state.0, ApplicationState::Connected) || !params.sim_ready.0 {
        move_pkt_state.initialized = false;
        move_pkt_state.ticks_since_pos = 0;
        move_pkt_state.last_sent_initialized = false;
        action_state.jump_was_pressed = false;
        action_state.fly_toggle_timer = 0;
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
    let mut sprinting_state = action_state.sprinting;
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

    sim_render.previous = sim_state.current;
    let next_state = if correction_guard.skip_physics_ticks > 0 {
        correction_guard.skip_physics_ticks = correction_guard.skip_physics_ticks.saturating_sub(1);
        let mut state = sim_state.current;
        state.pos = correction_guard.last_server_pos;
        state.on_ground = correction_guard.last_server_on_ground;
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
        if let Some((ack_pos, ack_yaw, ack_pitch, ack_on_ground)) =
            correction_guard.pending_acks.pop_front()
        {
            let _ = params.to_net.0.send(ToNetMessage::PlayerMovePosLook {
                x: ack_pos.0,
                y: ack_pos.1,
                z: ack_pos.2,
                yaw: ack_yaw,
                pitch: ack_pitch,
                on_ground: ack_on_ground,
            });
            log_outgoing_movement_packet(
                tick,
                "ack",
                MOVE_PKT_POS_LOOK,
                Vec3::new(ack_pos.0 as f32, ack_pos.1 as f32, ack_pos.2 as f32),
                ack_yaw,
                ack_pitch,
                ack_on_ground,
            );
            record_sent_movement_packet(
                &mut move_pkt_state,
                MOVE_PKT_POS_LOOK,
                Vec3::new(ack_pos.0 as f32, ack_pos.1 as f32, ack_pos.2 as f32),
                ack_yaw,
                ack_pitch,
                ack_on_ground,
            );
            latency.last_sent = Some(Instant::now());
            params.timings.fixed_tick_ms = timer.ms();
            return;
        }
        if correction_guard.skip_send_ticks > 0 {
            correction_guard.skip_send_ticks = correction_guard.skip_send_ticks.saturating_sub(1);
            params.timings.fixed_tick_ms = timer.ms();
            return;
        }
        if !has_loaded_player_chunk(&collision_map, sim_state.current.pos) {
            params.timings.fixed_tick_ms = timer.ms();
            return;
        }
        let current_sneak = input_snapshot.sneak;
        let current_sprint = effective_sprint(&input_snapshot);

        if current_sneak != action_state.sneaking {
            let action_id = if current_sneak { 0 } else { 1 };
            if let Some(entity_id) = params.remote_entities.local_entity_id {
                let _ = params.to_net.0.send(ToNetMessage::PlayerAction {
                    entity_id,
                    action_id,
                });
            }
            action_state.sneaking = current_sneak;
        }
        if current_sprint != action_state.sprinting {
            let action_id = if current_sprint { 3 } else { 4 };
            if let Some(entity_id) = params.remote_entities.local_entity_id {
                let _ = params.to_net.0.send(ToNetMessage::PlayerAction {
                    entity_id,
                    action_id,
                });
            }
            action_state.sprinting = current_sprint;
        }

        let pos = sim_state.current.pos;
        let mut yaw = wrap_degrees((std::f32::consts::PI - sim_state.current.yaw).to_degrees());
        let mut pitch = -sim_state.current.pitch.to_degrees();
        if !yaw.is_finite() {
            yaw = 0.0;
        }
        if !pitch.is_finite() {
            pitch = 0.0;
        }
        pitch = pitch.clamp(-90.0, 90.0);
        let on_ground = sim_state.current.on_ground;

        const POS_DELTA_SQ_EPS: f32 = 0.0009;

        let moved = if move_pkt_state.initialized {
            pos.distance_squared(move_pkt_state.last_pos) > POS_DELTA_SQ_EPS
                || move_pkt_state.ticks_since_pos >= 20
        } else {
            true
        };
        let rotated = if move_pkt_state.initialized {
            (yaw - move_pkt_state.last_yaw_deg).abs() > 0.001
                || (pitch - move_pkt_state.last_pitch_deg).abs() > 0.001
        } else {
            true
        };

        if moved && rotated {
            let _ = params.to_net.0.send(ToNetMessage::PlayerMovePosLook {
                x: pos.x as f64,
                y: pos.y as f64,
                z: pos.z as f64,
                yaw,
                pitch,
                on_ground,
            });
            log_outgoing_movement_packet(
                tick,
                "normal",
                MOVE_PKT_POS_LOOK,
                pos,
                yaw,
                pitch,
                on_ground,
            );
            record_sent_movement_packet(
                &mut move_pkt_state,
                MOVE_PKT_POS_LOOK,
                pos,
                yaw,
                pitch,
                on_ground,
            );
        } else if moved {
            let _ = params.to_net.0.send(ToNetMessage::PlayerMovePos {
                x: pos.x as f64,
                y: pos.y as f64,
                z: pos.z as f64,
                on_ground,
            });
            log_outgoing_movement_packet(tick, "normal", MOVE_PKT_POS, pos, yaw, pitch, on_ground);
            record_sent_movement_packet(
                &mut move_pkt_state,
                MOVE_PKT_POS,
                pos,
                yaw,
                pitch,
                on_ground,
            );
        } else if rotated {
            let _ = params.to_net.0.send(ToNetMessage::PlayerMoveLook {
                yaw,
                pitch,
                on_ground,
            });
            log_outgoing_movement_packet(tick, "normal", MOVE_PKT_LOOK, pos, yaw, pitch, on_ground);
            record_sent_movement_packet(
                &mut move_pkt_state,
                MOVE_PKT_LOOK,
                pos,
                yaw,
                pitch,
                on_ground,
            );
        } else {
            let _ = params
                .to_net
                .0
                .send(ToNetMessage::PlayerMoveGround { on_ground });
            log_outgoing_movement_packet(tick, "normal", MOVE_PKT_GROUND, pos, yaw, pitch, on_ground);
            record_sent_movement_packet(
                &mut move_pkt_state,
                MOVE_PKT_GROUND,
                pos,
                yaw,
                pitch,
                on_ground,
            );
        }

        if moved {
            move_pkt_state.last_pos = pos;
            move_pkt_state.ticks_since_pos = 0;
        } else {
            move_pkt_state.ticks_since_pos = move_pkt_state.ticks_since_pos.saturating_add(1);
        }
        move_pkt_state.last_yaw_deg = yaw;
        move_pkt_state.last_pitch_deg = pitch;
        move_pkt_state.initialized = true;
        latency.last_sent = Some(Instant::now());
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

pub fn net_event_apply_system(
    mut net_events: ResMut<NetEventQueue>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_state: ResMut<SimState>,
    mut history: ResMut<PredictionHistory>,
    mut visual_offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut latency: ResMut<LatencyEstimate>,
    mut correction_guard: ResMut<CorrectionLoopGuard>,
    mut move_pkt_state: ResMut<MovementPacketState>,
    mut sim_ready: ResMut<crate::sim::SimReady>,
    collision_map: Res<WorldCollisionMap>,
    sim_clock: Res<SimClock>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    let world = WorldCollision::with_map(&collision_map);
    for event in net_events.drain() {
        let (pos, ack_pos, yaw, pitch, on_ground, recv_instant) = match event {
            crate::net::events::NetEvent::ServerPosLook {
                pos,
                ack_pos,
                yaw,
                pitch,
                on_ground,
                recv_instant,
            } => (pos, ack_pos, yaw, pitch, on_ground, recv_instant),
            crate::net::events::NetEvent::ServerVelocity {
                velocity,
                recv_instant,
            } => {
                if let Some(last_sent) = latency.last_sent {
                    let rtt = recv_instant.saturating_duration_since(last_sent);
                    let one_way = rtt.as_secs_f32() * 0.5;
                    latency.one_way_ticks = (one_way / 0.05).round() as u32;
                    debug.one_way_ticks = latency.one_way_ticks;
                    debug.last_rtt_ms = rtt.as_secs_f32() * 1000.0;
                    debug.last_one_way_ms = one_way * 1000.0;
                }

                let latest_tick = sim_clock.tick.saturating_sub(1);
                let estimated_tick = latest_tick.saturating_sub(latency.one_way_ticks);
                let mut authoritative_state = history
                    .0
                    .get_by_tick(estimated_tick)
                    .map(|frame| frame.state)
                    .or_else(|| history.0.latest_frame().map(|frame| frame.state))
                    .unwrap_or(sim_state.current);
                authoritative_state.vel = velocity;
                let (server_tick, alignment_delta) = estimate_server_tick(
                    &history,
                    latest_tick,
                    estimated_tick,
                    &authoritative_state,
                );
                debug.last_tick_alignment_delta = alignment_delta;

                let previous_state = sim_state.current;
                if let Some(result) = reconcile(
                    &mut history.0,
                    &world,
                    server_tick,
                    authoritative_state,
                    latest_tick,
                    &mut sim_state.current,
                ) {
                    sim_render.previous = previous_state;
                    visual_offset.0 += previous_state.pos - sim_state.current.pos;
                    debug.last_correction = result.correction.length();
                    debug.last_replay = result.replayed_ticks;
                    debug.last_velocity_correction = result.velocity_correction;
                    debug.last_reconciled_server_tick = Some(server_tick);
                } else {
                    sim_state.current.vel = velocity;
                    if let Some(frame) = history.0.latest_frame_mut() {
                        frame.state = sim_state.current;
                    }
                    debug.last_velocity_correction = 0.0;
                    debug.last_reconciled_server_tick = Some(server_tick);
                }
                continue;
            }
        };
        if let Some(last_sent) = latency.last_sent {
            let rtt = recv_instant.saturating_duration_since(last_sent);
            let one_way = rtt.as_secs_f32() * 0.5;
            latency.one_way_ticks = (one_way / 0.05).round() as u32;
            debug.one_way_ticks = latency.one_way_ticks;
            debug.last_rtt_ms = rtt.as_secs_f32() * 1000.0;
            debug.last_one_way_ms = one_way * 1000.0;
        }

        let server_state = crate::sim::PlayerSimState {
            pos,
            vel: Vec3::ZERO,
            on_ground,
            collided_horizontally: false,
            jump_ticks: 0,
            yaw,
            pitch,
        };

        let repeated_same_correction = pos.distance_squared(correction_guard.last_server_pos)
            <= 1.0e-6
            && on_ground == correction_guard.last_server_on_ground;
        correction_guard.repeats = if repeated_same_correction {
            correction_guard.repeats.saturating_add(1)
        } else {
            0
        };
        correction_guard.last_server_pos = pos;
        correction_guard.last_server_on_ground = on_ground;
        correction_guard.skip_physics_ticks = 0;

        let resolved_yaw_deg = wrap_degrees((std::f32::consts::PI - yaw).to_degrees());
        let resolved_pitch_deg = (-pitch.to_degrees()).clamp(-90.0, 90.0);
        let ack_yaw_deg = resolved_yaw_deg;
        let ack_pitch_deg = resolved_pitch_deg;
        correction_guard.pending_acks.clear();
        correction_guard
            .pending_acks
            .push_back((ack_pos, ack_yaw_deg, ack_pitch_deg, on_ground));
        // Clientbound player position packets are authoritative teleports/setbacks.
        // Replaying buffered movement on top of them causes immediate divergence and
        // repeated anticheat setbacks, especially with Grim.
        let correction = pos - sim_state.current.pos;
        sim_render.previous = server_state;
        sim_state.current = server_state;
        history.0 = PredictionHistory::default().0;
        visual_offset.0 = Vec3::ZERO;
        debug.last_correction = correction.length();
        debug.last_replay = 0;
        debug.last_velocity_correction = 0.0;
        debug.last_reconciled_server_tick = None;
        sim_ready.0 = true;
        move_pkt_state.initialized = true;
        move_pkt_state.last_pos = sim_state.current.pos;
        move_pkt_state.last_yaw_deg = ack_yaw_deg;
        move_pkt_state.last_pitch_deg = ack_pitch_deg;
        move_pkt_state.ticks_since_pos = 0;
        correction_guard.skip_physics_ticks = 1;
        correction_guard.skip_send_ticks = 0;
    }
    timings.net_apply_ms = timer.ms();
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
