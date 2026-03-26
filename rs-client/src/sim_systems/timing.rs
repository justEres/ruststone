use super::*;

pub fn frame_timing_start(
    time: Res<Time>,
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    state.start = Some(Instant::now());
    timings.frame_delta_ms = time.delta_secs() * 1000.0;
}

pub fn frame_timing_end(mut state: ResMut<FrameTimingState>, mut timings: ResMut<PerfTimings>) {
    if let Some(start) = state.start.take() {
        timings.main_thread_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn performance_monitor_sample_system(
    timings: Res<PerfTimings>,
    render_perf: Res<RenderPerfStats>,
    mut monitor: ResMut<PerformanceMonitorState>,
    mut system: Local<Option<System>>,
) {
    let frame_ms = if timings.frame_delta_ms > 0.0 {
        timings.frame_delta_ms
    } else {
        timings.main_thread_ms
    };
    let render_ms = (render_perf.last_apply_ms
        + render_perf.last_enqueue_ms
        + render_perf.occlusion_cull_ms
        + render_perf.apply_debug_ms
        + render_perf.gather_stats_ms)
        .max(0.0);

    let system = system.get_or_insert_with(System::new);
    let (cpu_percent, ram_mb) = if let Some(pid) = monitor.pid {
        let _ = system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        if let Some(process) = system.process(pid) {
            (
                process.cpu_usage().max(0.0),
                process.memory() as f32 / (1024.0 * 1024.0),
            )
        } else {
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    };

    if monitor.samples.len() >= monitor.max_samples {
        monitor.samples.pop_front();
    }
    monitor.samples.push_back(PerfMonitorSample {
        frame_ms,
        cpu_percent,
        ram_mb,
        render_ms,
    });
}

pub fn update_timing_start(mut state: ResMut<FrameTimingState>) {
    state.update_start = Some(Instant::now());
}

pub fn update_timing_end(mut state: ResMut<FrameTimingState>, mut timings: ResMut<PerfTimings>) {
    if let Some(start) = state.update_start.take() {
        timings.update_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn post_update_timing_start(mut state: ResMut<FrameTimingState>) {
    state.post_update_start = Some(Instant::now());
}

pub fn post_update_timing_end(
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    if let Some(start) = state.post_update_start.take() {
        timings.post_update_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn fixed_update_timing_start(mut state: ResMut<FrameTimingState>) {
    state.fixed_update_start = Some(Instant::now());
}

pub fn fixed_update_timing_end(
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    if let Some(start) = state.fixed_update_start.take() {
        timings.fixed_update_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}
