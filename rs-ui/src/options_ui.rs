use bevy::prelude::Window;
use bevy::window::PresentMode;
use bevy_egui::egui;
use rs_render::{AntiAliasingMode, RenderDebugSettings, ShadingModel, VanillaBlockShadowMode};
use rs_utils::SoundSettings;

use super::{ConnectUiState, load_client_options, save_client_options};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsCategory {
    General,
    Lighting,
    Water,
    Sound,
    ChatHud,
    Diagnostics,
    System,
}

impl SettingsCategory {
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Lighting,
        Self::Water,
        Self::Sound,
        Self::ChatHud,
        Self::Diagnostics,
        Self::System,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Lighting => "Lighting & Shadows",
            Self::Water => "Water",
            Self::Sound => "Sound",
            Self::ChatHud => "Chat & HUD",
            Self::Diagnostics => "Diagnostics",
            Self::System => "System",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::General => 0,
            Self::Lighting => 1,
            Self::Water => 2,
            Self::Sound => 3,
            Self::ChatHud => 4,
            Self::Diagnostics => 5,
            Self::System => 6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsCategoryState {
    open: [bool; 7],
}

impl Default for SettingsCategoryState {
    fn default() -> Self {
        Self { open: [false; 7] }
    }
}

impl SettingsCategoryState {
    pub fn is_open(&self, category: SettingsCategory) -> bool {
        self.open[category.index()]
    }

    pub fn set_open(&mut self, category: SettingsCategory, open: bool) {
        self.open[category.index()] = open;
    }
}

#[derive(Clone, Copy)]
struct SettingEntry {
    id: SettingId,
    title: &'static str,
    category: SettingsCategory,
    aliases: &'static [&'static str],
    visible: fn(&ConnectUiState, &RenderDebugSettings, &SoundSettings) -> bool,
}

#[derive(Clone, Copy)]
enum SettingId {
    Fov,
    SimulationDistance,
    RenderDistance,
    InfiniteRenderDistance,
    FlightSpeedBoostEnabled,
    FlightSpeedBoostMultiplier,
    AntiAliasing,
    OcclusionCull,
    OcclusionAnchorPlayer,
    CullGuardRadius,
    UseGreedyMeshing,
    Wireframe,
    BarrierBillboard,
    RenderHeldItems,
    RenderFirstPersonArms,
    RenderSelfModel,
    Vsync,
    ShadingPreset,
    SyncSunWithTime,
    RenderSunSprite,
    SunAzimuth,
    SunElevation,
    SunStrength,
    SunWarmth,
    AmbientStrength,
    VoxelAo,
    VoxelAoCutout,
    VoxelAoStrength,
    FogEnabled,
    FogIntensity,
    FogDensity,
    FogStart,
    FogEnd,
    ColorSaturation,
    ColorContrast,
    ColorBrightness,
    ColorGamma,
    VanillaSkyLightStrength,
    VanillaBlockLightStrength,
    VanillaFaceShadingStrength,
    VanillaAmbientFloor,
    VanillaLightCurve,
    VanillaFoliageTintStrength,
    VanillaBlockShadowMode,
    VanillaBlockShadowStrength,
    VanillaSunTraceSamples,
    VanillaSunTraceDistance,
    VanillaTopFaceSunBias,
    VanillaAoShadowBlend,
    ShadowsEnabled,
    ShadowDistanceScale,
    ShadowMapSize,
    ShadowCascades,
    ShadowMaxDistance,
    ShadowOpacity,
    PlayerShadowOpacity,
    WaterReflectionsEnabled,
    WaterReflectionStrength,
    WaterReflectionNearBoost,
    WaterReflectionBlueTint,
    WaterReflectionTintStrength,
    WaterWaveStrength,
    WaterWaveSpeed,
    WaterWaveDetailStrength,
    WaterWaveDetailScale,
    WaterWaveDetailSpeed,
    WaterReflectionEdgeFade,
    WaterReflectionSkyFill,
    WaterReflectionScreenSpace,
    WaterSsrSteps,
    WaterSsrThickness,
    WaterSsrMaxDistance,
    WaterSsrStride,
    SoundMaster,
    SoundMusic,
    SoundRecord,
    SoundWeather,
    SoundBlock,
    SoundHostile,
    SoundNeutral,
    SoundPlayer,
    SoundAmbient,
    ChatBackgroundOpacity,
    ChatFontSize,
    ScoreboardBackgroundOpacity,
    ScoreboardFontSize,
    TitleBackgroundOpacity,
    TitleFontSize,
    ShaderDebugView,
    FrustumFovDebug,
    FrustumFov,
    ShowChunkBorders,
    MeshJobsPerFrame,
    MeshUploadsPerFrame,
    MaxAsyncMeshing,
}

fn always(_: &ConnectUiState, _: &RenderDebugSettings, _: &SoundSettings) -> bool {
    true
}

fn when_flight_boost(_: &ConnectUiState, render: &RenderDebugSettings, _: &SoundSettings) -> bool {
    render.flight_speed_boost_enabled
}

fn sun_manual(_: &ConnectUiState, render: &RenderDebugSettings, _: &SoundSettings) -> bool {
    !render.sync_sun_with_time
}

fn vanilla_only(_: &ConnectUiState, render: &RenderDebugSettings, _: &SoundSettings) -> bool {
    render.shading_model == ShadingModel::VanillaLighting
}

fn pbr_only(_: &ConnectUiState, render: &RenderDebugSettings, _: &SoundSettings) -> bool {
    render.shading_model == ShadingModel::PbrFancy
}

fn ssr_toggle_visible(
    _: &ConnectUiState,
    render: &RenderDebugSettings,
    _: &SoundSettings,
) -> bool {
    render.shading_model == ShadingModel::PbrFancy
}

fn ssr_detail_visible(
    _: &ConnectUiState,
    render: &RenderDebugSettings,
    _: &SoundSettings,
) -> bool {
    render.shading_model == ShadingModel::PbrFancy && render.water_reflection_screen_space
}

fn frustum_fov_visible(
    _: &ConnectUiState,
    render: &RenderDebugSettings,
    _: &SoundSettings,
) -> bool {
    render.frustum_fov_debug
}

const SETTINGS: &[SettingEntry] = &[
    SettingEntry { id: SettingId::Fov, title: "FOV", category: SettingsCategory::General, aliases: &["field of view"], visible: always },
    SettingEntry { id: SettingId::SimulationDistance, title: "Simulation Distance", category: SettingsCategory::General, aliases: &["server distance", "collision distance", "simulation chunks"], visible: always },
    SettingEntry { id: SettingId::RenderDistance, title: "Render Distance", category: SettingsCategory::General, aliases: &["chunks", "visual distance"], visible: always },
    SettingEntry { id: SettingId::InfiniteRenderDistance, title: "Infinite local render distance", category: SettingsCategory::General, aliases: &["infinite", "unlimited render distance"], visible: always },
    SettingEntry { id: SettingId::FlightSpeedBoostEnabled, title: "Boost creative flight speed", category: SettingsCategory::General, aliases: &["flight speed", "creative speed"], visible: always },
    SettingEntry { id: SettingId::FlightSpeedBoostMultiplier, title: "Flight speed multiplier", category: SettingsCategory::General, aliases: &["flight multiplier"], visible: when_flight_boost },
    SettingEntry { id: SettingId::AntiAliasing, title: "Anti-aliasing", category: SettingsCategory::General, aliases: &["aa", "msaa", "fxaa", "smaa"], visible: always },
    SettingEntry { id: SettingId::OcclusionCull, title: "Occlusion cull", category: SettingsCategory::General, aliases: &["occlusion", "culling"], visible: always },
    SettingEntry { id: SettingId::OcclusionAnchorPlayer, title: "Anchor occlusion to player", category: SettingsCategory::General, aliases: &["occlusion anchor"], visible: always },
    SettingEntry { id: SettingId::CullGuardRadius, title: "Cull guard radius (chunks)", category: SettingsCategory::General, aliases: &["cull guard"], visible: always },
    SettingEntry { id: SettingId::UseGreedyMeshing, title: "Binary greedy meshing", category: SettingsCategory::General, aliases: &["greedy meshing", "meshing"], visible: always },
    SettingEntry { id: SettingId::Wireframe, title: "Wireframe", category: SettingsCategory::General, aliases: &[], visible: always },
    SettingEntry { id: SettingId::BarrierBillboard, title: "Barriers as billboard sprites", category: SettingsCategory::General, aliases: &["barriers", "billboard"], visible: always },
    SettingEntry { id: SettingId::RenderHeldItems, title: "Render held items", category: SettingsCategory::General, aliases: &["held items"], visible: always },
    SettingEntry { id: SettingId::RenderFirstPersonArms, title: "Render first-person arms", category: SettingsCategory::General, aliases: &["arms", "first person"], visible: always },
    SettingEntry { id: SettingId::RenderSelfModel, title: "Render self model", category: SettingsCategory::General, aliases: &["self model"], visible: always },
    SettingEntry { id: SettingId::Vsync, title: "VSync", category: SettingsCategory::General, aliases: &["vsync", "vertical sync"], visible: always },
    SettingEntry { id: SettingId::ShadingPreset, title: "Shading preset", category: SettingsCategory::Lighting, aliases: &["shading", "lighting mode", "preset"], visible: always },
    SettingEntry { id: SettingId::SyncSunWithTime, title: "Sync sun with world time", category: SettingsCategory::Lighting, aliases: &["sun sync", "time"], visible: always },
    SettingEntry { id: SettingId::RenderSunSprite, title: "Render sun sprite", category: SettingsCategory::Lighting, aliases: &["sun sprite"], visible: always },
    SettingEntry { id: SettingId::SunAzimuth, title: "Sun azimuth", category: SettingsCategory::Lighting, aliases: &["sun angle"], visible: sun_manual },
    SettingEntry { id: SettingId::SunElevation, title: "Sun elevation", category: SettingsCategory::Lighting, aliases: &["sun height"], visible: sun_manual },
    SettingEntry { id: SettingId::SunStrength, title: "Sun strength", category: SettingsCategory::Lighting, aliases: &["sun"], visible: always },
    SettingEntry { id: SettingId::SunWarmth, title: "Sun warmth", category: SettingsCategory::Lighting, aliases: &["warmth"], visible: always },
    SettingEntry { id: SettingId::AmbientStrength, title: "Ambient strength", category: SettingsCategory::Lighting, aliases: &["ambient"], visible: always },
    SettingEntry { id: SettingId::VoxelAo, title: "Voxel AO", category: SettingsCategory::Lighting, aliases: &["ambient occlusion", "ao"], visible: always },
    SettingEntry { id: SettingId::VoxelAoCutout, title: "Voxel AO on cutout blocks", category: SettingsCategory::Lighting, aliases: &["cutout ao"], visible: always },
    SettingEntry { id: SettingId::VoxelAoStrength, title: "Voxel AO strength", category: SettingsCategory::Lighting, aliases: &["ao strength"], visible: always },
    SettingEntry { id: SettingId::FogEnabled, title: "Fog enabled", category: SettingsCategory::Lighting, aliases: &["fog"], visible: always },
    SettingEntry { id: SettingId::FogIntensity, title: "Fog intensity", category: SettingsCategory::Lighting, aliases: &[], visible: always },
    SettingEntry { id: SettingId::FogDensity, title: "Fog density", category: SettingsCategory::Lighting, aliases: &[], visible: always },
    SettingEntry { id: SettingId::FogStart, title: "Fog start", category: SettingsCategory::Lighting, aliases: &[], visible: always },
    SettingEntry { id: SettingId::FogEnd, title: "Fog end", category: SettingsCategory::Lighting, aliases: &[], visible: always },
    SettingEntry { id: SettingId::ColorSaturation, title: "Saturation", category: SettingsCategory::Lighting, aliases: &["color saturation"], visible: always },
    SettingEntry { id: SettingId::ColorContrast, title: "Contrast", category: SettingsCategory::Lighting, aliases: &["color contrast"], visible: always },
    SettingEntry { id: SettingId::ColorBrightness, title: "Brightness", category: SettingsCategory::Lighting, aliases: &["color brightness"], visible: always },
    SettingEntry { id: SettingId::ColorGamma, title: "Gamma", category: SettingsCategory::Lighting, aliases: &["gamma"], visible: always },
    SettingEntry { id: SettingId::VanillaSkyLightStrength, title: "Sky light strength", category: SettingsCategory::Lighting, aliases: &["sky light"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaBlockLightStrength, title: "Block light strength", category: SettingsCategory::Lighting, aliases: &["block light"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaFaceShadingStrength, title: "Face shading strength", category: SettingsCategory::Lighting, aliases: &["face shading"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaAmbientFloor, title: "Ambient floor", category: SettingsCategory::Lighting, aliases: &["ambient floor"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaLightCurve, title: "Light curve", category: SettingsCategory::Lighting, aliases: &["curve"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaFoliageTintStrength, title: "Foliage tint strength", category: SettingsCategory::Lighting, aliases: &["leaf tint", "biome tint", "foliage tint"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaBlockShadowMode, title: "Block shadow mode", category: SettingsCategory::Lighting, aliases: &["block shadows"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaBlockShadowStrength, title: "Block shadow strength", category: SettingsCategory::Lighting, aliases: &["shadow strength"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaSunTraceSamples, title: "Sun trace samples", category: SettingsCategory::Lighting, aliases: &["sun trace"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaSunTraceDistance, title: "Sun trace distance", category: SettingsCategory::Lighting, aliases: &["trace distance"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaTopFaceSunBias, title: "Top-face sun bias", category: SettingsCategory::Lighting, aliases: &["sun bias"], visible: vanilla_only },
    SettingEntry { id: SettingId::VanillaAoShadowBlend, title: "AO/shadow blend", category: SettingsCategory::Lighting, aliases: &["ao shadow blend"], visible: vanilla_only },
    SettingEntry { id: SettingId::ShadowsEnabled, title: "Shadows", category: SettingsCategory::Lighting, aliases: &["shadow"], visible: pbr_only },
    SettingEntry { id: SettingId::ShadowDistanceScale, title: "Shadow distance", category: SettingsCategory::Lighting, aliases: &[], visible: pbr_only },
    SettingEntry { id: SettingId::ShadowMapSize, title: "Shadow map size", category: SettingsCategory::Lighting, aliases: &["shadow resolution"], visible: pbr_only },
    SettingEntry { id: SettingId::ShadowCascades, title: "Shadow cascades", category: SettingsCategory::Lighting, aliases: &[], visible: pbr_only },
    SettingEntry { id: SettingId::ShadowMaxDistance, title: "Shadow max distance", category: SettingsCategory::Lighting, aliases: &[], visible: pbr_only },
    SettingEntry { id: SettingId::ShadowOpacity, title: "Shadow opacity", category: SettingsCategory::Lighting, aliases: &[], visible: pbr_only },
    SettingEntry { id: SettingId::PlayerShadowOpacity, title: "Player shadow opacity", category: SettingsCategory::Lighting, aliases: &[], visible: pbr_only },
    SettingEntry { id: SettingId::WaterReflectionsEnabled, title: "Water reflections", category: SettingsCategory::Water, aliases: &["water"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionStrength, title: "Water reflection strength", category: SettingsCategory::Water, aliases: &["reflection strength"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionNearBoost, title: "Near reflection boost", category: SettingsCategory::Water, aliases: &["near boost"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionBlueTint, title: "Blue reflection tint", category: SettingsCategory::Water, aliases: &["blue tint"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionTintStrength, title: "Blue tint strength", category: SettingsCategory::Water, aliases: &["tint strength"], visible: always },
    SettingEntry { id: SettingId::WaterWaveStrength, title: "Water wave strength", category: SettingsCategory::Water, aliases: &["wave strength"], visible: always },
    SettingEntry { id: SettingId::WaterWaveSpeed, title: "Water wave speed", category: SettingsCategory::Water, aliases: &["wave speed"], visible: always },
    SettingEntry { id: SettingId::WaterWaveDetailStrength, title: "Water detail wave strength", category: SettingsCategory::Water, aliases: &["detail wave strength"], visible: always },
    SettingEntry { id: SettingId::WaterWaveDetailScale, title: "Water detail wave scale", category: SettingsCategory::Water, aliases: &["detail wave scale"], visible: always },
    SettingEntry { id: SettingId::WaterWaveDetailSpeed, title: "Water detail wave speed", category: SettingsCategory::Water, aliases: &["detail wave speed"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionEdgeFade, title: "Reflection edge fade", category: SettingsCategory::Water, aliases: &["edge fade"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionSkyFill, title: "Reflection sky fallback", category: SettingsCategory::Water, aliases: &["sky fallback"], visible: always },
    SettingEntry { id: SettingId::WaterReflectionScreenSpace, title: "SSR reflections", category: SettingsCategory::Water, aliases: &["ssr"], visible: ssr_toggle_visible },
    SettingEntry { id: SettingId::WaterSsrSteps, title: "SSR ray steps", category: SettingsCategory::Water, aliases: &["ssr steps"], visible: ssr_detail_visible },
    SettingEntry { id: SettingId::WaterSsrThickness, title: "SSR hit thickness", category: SettingsCategory::Water, aliases: &["ssr thickness"], visible: ssr_detail_visible },
    SettingEntry { id: SettingId::WaterSsrMaxDistance, title: "SSR max distance", category: SettingsCategory::Water, aliases: &["ssr distance"], visible: ssr_detail_visible },
    SettingEntry { id: SettingId::WaterSsrStride, title: "SSR step stride", category: SettingsCategory::Water, aliases: &["ssr stride"], visible: ssr_detail_visible },
    SettingEntry { id: SettingId::SoundMaster, title: "Master volume", category: SettingsCategory::Sound, aliases: &["master"], visible: always },
    SettingEntry { id: SettingId::SoundMusic, title: "Music volume", category: SettingsCategory::Sound, aliases: &["music"], visible: always },
    SettingEntry { id: SettingId::SoundRecord, title: "Record volume", category: SettingsCategory::Sound, aliases: &["records"], visible: always },
    SettingEntry { id: SettingId::SoundWeather, title: "Weather volume", category: SettingsCategory::Sound, aliases: &["weather"], visible: always },
    SettingEntry { id: SettingId::SoundBlock, title: "Block volume", category: SettingsCategory::Sound, aliases: &["block sounds"], visible: always },
    SettingEntry { id: SettingId::SoundHostile, title: "Hostile volume", category: SettingsCategory::Sound, aliases: &["hostile"], visible: always },
    SettingEntry { id: SettingId::SoundNeutral, title: "Neutral volume", category: SettingsCategory::Sound, aliases: &["neutral"], visible: always },
    SettingEntry { id: SettingId::SoundPlayer, title: "Player volume", category: SettingsCategory::Sound, aliases: &["player sounds"], visible: always },
    SettingEntry { id: SettingId::SoundAmbient, title: "Ambient volume", category: SettingsCategory::Sound, aliases: &["ambient sounds"], visible: always },
    SettingEntry { id: SettingId::ChatBackgroundOpacity, title: "Chat background opacity", category: SettingsCategory::ChatHud, aliases: &["chat background"], visible: always },
    SettingEntry { id: SettingId::ChatFontSize, title: "Chat font size", category: SettingsCategory::ChatHud, aliases: &["chat font"], visible: always },
    SettingEntry { id: SettingId::ScoreboardBackgroundOpacity, title: "Scoreboard background opacity", category: SettingsCategory::ChatHud, aliases: &["scoreboard background"], visible: always },
    SettingEntry { id: SettingId::ScoreboardFontSize, title: "Scoreboard font size", category: SettingsCategory::ChatHud, aliases: &["scoreboard font"], visible: always },
    SettingEntry { id: SettingId::TitleBackgroundOpacity, title: "Title background opacity", category: SettingsCategory::ChatHud, aliases: &["title background"], visible: always },
    SettingEntry { id: SettingId::TitleFontSize, title: "Title font size", category: SettingsCategory::ChatHud, aliases: &["title font"], visible: always },
    SettingEntry { id: SettingId::ShaderDebugView, title: "Shader debug view", category: SettingsCategory::Diagnostics, aliases: &["shader debug"], visible: always },
    SettingEntry { id: SettingId::FrustumFovDebug, title: "Frustum FOV debug", category: SettingsCategory::Diagnostics, aliases: &["frustum"], visible: always },
    SettingEntry { id: SettingId::FrustumFov, title: "Frustum FOV", category: SettingsCategory::Diagnostics, aliases: &["frustum fov"], visible: frustum_fov_visible },
    SettingEntry { id: SettingId::ShowChunkBorders, title: "Show chunk borders", category: SettingsCategory::Diagnostics, aliases: &["chunk borders"], visible: always },
    SettingEntry { id: SettingId::MeshJobsPerFrame, title: "Mesh jobs per frame", category: SettingsCategory::Diagnostics, aliases: &["mesh jobs"], visible: always },
    SettingEntry { id: SettingId::MeshUploadsPerFrame, title: "Mesh uploads per frame", category: SettingsCategory::Diagnostics, aliases: &["mesh uploads"], visible: always },
    SettingEntry { id: SettingId::MaxAsyncMeshing, title: "Max async meshing", category: SettingsCategory::Diagnostics, aliases: &["async meshing"], visible: always },
];

pub fn render_settings_panel(
    ui: &mut egui::Ui,
    state: &mut ConnectUiState,
    render_debug: &mut RenderDebugSettings,
    sound_settings: &mut SoundSettings,
    primary_window: &mut Option<bevy::ecs::change_detection::Mut<'_, Window>>,
) -> bool {
    let mut options_changed = false;

    ui.horizontal(|ui| {
        ui.label("Search");
        ui.add(egui::TextEdit::singleline(&mut state.options_search).hint_text("Search options"));
        if ui.button("Clear").clicked() {
            state.options_search.clear();
        }
    });

    let query = state.options_search.trim().to_ascii_lowercase();
    ui.add_space(8.0);
    if query.is_empty() {
        for category in SettingsCategory::ALL {
            if !category_has_visible_content(category, state, render_debug, sound_settings) {
                continue;
            }
            let header = egui::CollapsingHeader::new(category.label())
                .default_open(state.settings_category_state.is_open(category))
                .show(ui, |ui| {
                    options_changed |= render_category_settings(
                        category,
                        ui,
                        state,
                        render_debug,
                        sound_settings,
                        primary_window,
                    );
                    render_category_extras(
                        category,
                        ui,
                        state,
                        render_debug,
                        sound_settings,
                        primary_window,
                    );
                });
            state
                .settings_category_state
                .set_open(category, header.fully_open());
        }
    } else {
        let matches: Vec<SettingEntry> = SETTINGS
            .iter()
            .copied()
            .filter(|entry| {
                (entry.visible)(state, render_debug, sound_settings)
                    && setting_matches_query(*entry, &query)
            })
            .collect();
        if matches.is_empty() {
            ui.label("No matching settings.");
        } else {
            for category in SettingsCategory::ALL {
                let category_matches: Vec<SettingEntry> = matches
                    .iter()
                    .copied()
                    .filter(|entry| entry.category == category)
                    .collect();
                if category_matches.is_empty() {
                    continue;
                }
                ui.heading(category.label());
                ui.add_space(4.0);
                for entry in category_matches {
                    options_changed |= render_setting_row(
                        entry.id,
                        ui,
                        state,
                        render_debug,
                        sound_settings,
                        primary_window,
                    );
                }
                ui.add_space(8.0);
            }
        }
    }

    options_changed
}

fn category_has_visible_content(
    category: SettingsCategory,
    state: &ConnectUiState,
    render_debug: &RenderDebugSettings,
    sound_settings: &SoundSettings,
) -> bool {
    SETTINGS
        .iter()
        .any(|entry| entry.category == category && (entry.visible)(state, render_debug, sound_settings))
        || matches!(category, SettingsCategory::Diagnostics | SettingsCategory::System)
}

fn render_category_settings(
    category: SettingsCategory,
    ui: &mut egui::Ui,
    state: &mut ConnectUiState,
    render_debug: &mut RenderDebugSettings,
    sound_settings: &mut SoundSettings,
    primary_window: &mut Option<bevy::ecs::change_detection::Mut<'_, Window>>,
) -> bool {
    let mut changed = false;
    for entry in SETTINGS.iter().copied().filter(|entry| entry.category == category) {
        if !(entry.visible)(state, render_debug, sound_settings) {
            continue;
        }
        changed |= render_setting_row(
            entry.id,
            ui,
            state,
            render_debug,
            sound_settings,
            primary_window,
        );
    }
    changed
}

fn setting_matches_query(entry: SettingEntry, query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    entry.title.to_ascii_lowercase().contains(&query)
        || entry
            .aliases
            .iter()
            .any(|alias| alias.to_ascii_lowercase().contains(&query))
}

fn render_setting_row(
    id: SettingId,
    ui: &mut egui::Ui,
    state: &mut ConnectUiState,
    render_debug: &mut RenderDebugSettings,
    sound_settings: &mut SoundSettings,
    primary_window: &mut Option<bevy::ecs::change_detection::Mut<'_, Window>>,
) -> bool {
    match id {
        SettingId::Fov => ui.add(egui::Slider::new(&mut render_debug.fov_deg, 60.0..=140.0).text("FOV")).changed(),
        SettingId::SimulationDistance => {
            let changed = ui
                .add(egui::Slider::new(&mut render_debug.simulation_distance_chunks, 2..=64).text("Simulation Distance"))
                .changed();
            ui.label("Controls the server chunk request distance and the local collision/simulation radius. Reconnect to change what the server streams.");
            changed
        }
        SettingId::RenderDistance => {
            let changed = ui
                .add(egui::Slider::new(&mut render_debug.render_distance_chunks, 2..=64).text("Render Distance"))
                .changed();
            ui.label("Controls local visibility only. With infinite local render distance enabled, this only affects normal near-chunk visibility.");
            changed
        }
        SettingId::InfiniteRenderDistance => {
            let changed = ui.checkbox(&mut render_debug.infinite_render_distance, "Infinite local render distance").changed();
            ui.label("Keeps received chunk meshes locally after they leave the simulation radius and bypasses local occlusion culling. Collision is still pruned to the simulation distance.");
            changed
        }
        SettingId::FlightSpeedBoostEnabled => ui.checkbox(&mut render_debug.flight_speed_boost_enabled, "Boost creative flight speed").changed(),
        SettingId::FlightSpeedBoostMultiplier => ui.add(egui::Slider::new(&mut render_debug.flight_speed_boost_multiplier, 1.0..=10.0).text("Flight speed multiplier")).changed(),
        SettingId::AntiAliasing => {
            let mut selected = render_debug.aa_mode;
            egui::ComboBox::from_label("Anti-aliasing")
                .selected_text(selected.label())
                .show_ui(ui, |ui| {
                    for mode in AntiAliasingMode::ALL {
                        ui.selectable_value(&mut selected, mode, mode.label());
                    }
                });
            if selected != render_debug.aa_mode {
                render_debug.aa_mode = selected;
                true
            } else {
                false
            }
        }
        SettingId::OcclusionCull => ui.checkbox(&mut render_debug.occlusion_cull_enabled, "Occlusion cull").changed(),
        SettingId::OcclusionAnchorPlayer => ui.checkbox(&mut render_debug.occlusion_anchor_player, "Anchor occlusion to player").changed(),
        SettingId::CullGuardRadius => ui.add(egui::Slider::new(&mut render_debug.cull_guard_chunk_radius, 0..=5).text("Cull guard radius (chunks)")).changed(),
        SettingId::UseGreedyMeshing => ui.checkbox(&mut render_debug.use_greedy_meshing, "Binary greedy meshing").changed(),
        SettingId::Wireframe => ui.checkbox(&mut render_debug.wireframe_enabled, "Wireframe").changed(),
        SettingId::BarrierBillboard => ui.checkbox(&mut render_debug.barrier_billboard, "Barriers as billboard sprites").changed(),
        SettingId::RenderHeldItems => ui.checkbox(&mut render_debug.render_held_items, "Render held items").changed(),
        SettingId::RenderFirstPersonArms => ui.checkbox(&mut render_debug.render_first_person_arms, "Render first-person arms").changed(),
        SettingId::RenderSelfModel => ui.checkbox(&mut render_debug.render_self_model, "Render self model").changed(),
        SettingId::Vsync => {
            let changed = ui.checkbox(&mut state.vsync_enabled, "VSync").changed();
            if changed {
                if let Some(window) = primary_window.as_deref_mut() {
                    window.present_mode = if state.vsync_enabled {
                        PresentMode::AutoVsync
                    } else {
                        PresentMode::AutoNoVsync
                    };
                }
            }
            changed
        }
        SettingId::ShadingPreset => {
            let mut selected = render_debug.shading_model;
            egui::ComboBox::from_label("Shading preset")
                .selected_text(selected.label())
                .show_ui(ui, |ui| {
                    for mode in ShadingModel::ALL {
                        ui.selectable_value(&mut selected, mode, mode.label());
                    }
                });
            if selected != render_debug.shading_model {
                render_debug.shading_model = selected;
                true
            } else {
                false
            }
        }
        SettingId::SyncSunWithTime => ui.checkbox(&mut render_debug.sync_sun_with_time, "Sync sun with world time").changed(),
        SettingId::RenderSunSprite => ui.checkbox(&mut render_debug.render_sun_sprite, "Render sun sprite").changed(),
        SettingId::SunAzimuth => ui.add(egui::Slider::new(&mut render_debug.sun_azimuth_deg, -180.0..=180.0).text("Sun azimuth")).changed(),
        SettingId::SunElevation => ui.add(egui::Slider::new(&mut render_debug.sun_elevation_deg, -20.0..=89.0).text("Sun elevation")).changed(),
        SettingId::SunStrength => ui.add(egui::Slider::new(&mut render_debug.sun_strength, 0.0..=2.0).text("Sun strength")).changed(),
        SettingId::SunWarmth => ui.add(egui::Slider::new(&mut render_debug.sun_warmth, 0.0..=1.0).text("Sun warmth")).changed(),
        SettingId::AmbientStrength => ui.add(egui::Slider::new(&mut render_debug.ambient_strength, 0.0..=2.0).text("Ambient strength")).changed(),
        SettingId::VoxelAo => ui.checkbox(&mut render_debug.voxel_ao_enabled, "Voxel AO").changed(),
        SettingId::VoxelAoCutout => ui.checkbox(&mut render_debug.voxel_ao_cutout, "Voxel AO on cutout blocks").changed(),
        SettingId::VoxelAoStrength => ui.add(egui::Slider::new(&mut render_debug.voxel_ao_strength, 0.0..=1.0).text("Voxel AO strength")).changed(),
        SettingId::FogEnabled => ui.checkbox(&mut render_debug.fog_enabled, "Fog enabled").changed(),
        SettingId::FogIntensity => ui.add(egui::Slider::new(&mut render_debug.fog_intensity, 0.0..=2.0).text("Fog intensity")).changed(),
        SettingId::FogDensity => ui.add(egui::Slider::new(&mut render_debug.fog_density, 0.0..=0.08).text("Fog density")).changed(),
        SettingId::FogStart => ui.add(egui::Slider::new(&mut render_debug.fog_start, 0.0..=400.0).text("Fog start")).changed(),
        SettingId::FogEnd => ui.add(egui::Slider::new(&mut render_debug.fog_end, 1.0..=600.0).text("Fog end")).changed(),
        SettingId::ColorSaturation => ui.add(egui::Slider::new(&mut render_debug.color_saturation, 0.5..=1.8).text("Saturation")).changed(),
        SettingId::ColorContrast => ui.add(egui::Slider::new(&mut render_debug.color_contrast, 0.6..=1.6).text("Contrast")).changed(),
        SettingId::ColorBrightness => ui.add(egui::Slider::new(&mut render_debug.color_brightness, -0.2..=0.2).text("Brightness")).changed(),
        SettingId::ColorGamma => ui.add(egui::Slider::new(&mut render_debug.color_gamma, 0.6..=1.8).text("Gamma")).changed(),
        SettingId::VanillaSkyLightStrength => ui.add(egui::Slider::new(&mut render_debug.vanilla_sky_light_strength, 0.0..=2.0).text("Sky light strength")).changed(),
        SettingId::VanillaBlockLightStrength => ui.add(egui::Slider::new(&mut render_debug.vanilla_block_light_strength, 0.0..=2.0).text("Block light strength")).changed(),
        SettingId::VanillaFaceShadingStrength => ui.add(egui::Slider::new(&mut render_debug.vanilla_face_shading_strength, 0.0..=1.0).text("Face shading strength")).changed(),
        SettingId::VanillaAmbientFloor => ui.add(egui::Slider::new(&mut render_debug.vanilla_ambient_floor, 0.0..=0.95).text("Ambient floor")).changed(),
        SettingId::VanillaLightCurve => ui.add(egui::Slider::new(&mut render_debug.vanilla_light_curve, 0.35..=2.5).text("Light curve")).changed(),
        SettingId::VanillaFoliageTintStrength => ui.add(egui::Slider::new(&mut render_debug.vanilla_foliage_tint_strength, 0.0..=2.5).text("Foliage tint strength")).changed(),
        SettingId::VanillaBlockShadowMode => {
            let mut selected = render_debug.vanilla_block_shadow_mode;
            egui::ComboBox::from_label("Block shadow mode")
                .selected_text(selected.label())
                .show_ui(ui, |ui| {
                    for mode in VanillaBlockShadowMode::ALL {
                        ui.selectable_value(&mut selected, mode, mode.label());
                    }
                });
            if selected != render_debug.vanilla_block_shadow_mode {
                render_debug.vanilla_block_shadow_mode = selected;
                true
            } else {
                false
            }
        }
        SettingId::VanillaBlockShadowStrength => ui.add(egui::Slider::new(&mut render_debug.vanilla_block_shadow_strength, 0.0..=1.0).text("Block shadow strength")).changed(),
        SettingId::VanillaSunTraceSamples => ui.add(egui::Slider::new(&mut render_debug.vanilla_sun_trace_samples, 1..=8).text("Sun trace samples")).changed(),
        SettingId::VanillaSunTraceDistance => ui.add(egui::Slider::new(&mut render_debug.vanilla_sun_trace_distance, 1.0..=12.0).text("Sun trace distance")).changed(),
        SettingId::VanillaTopFaceSunBias => ui.add(egui::Slider::new(&mut render_debug.vanilla_top_face_sun_bias, 0.0..=0.5).text("Top-face sun bias")).changed(),
        SettingId::VanillaAoShadowBlend => ui.add(egui::Slider::new(&mut render_debug.vanilla_ao_shadow_blend, 0.0..=1.0).text("AO/shadow blend")).changed(),
        SettingId::ShadowsEnabled => ui.checkbox(&mut render_debug.shadows_enabled, "Shadows").changed(),
        SettingId::ShadowDistanceScale => ui.add(egui::Slider::new(&mut render_debug.shadow_distance_scale, 0.25..=20.0).logarithmic(true).text("Shadow distance")).changed(),
        SettingId::ShadowMapSize => ui.add(egui::Slider::new(&mut render_debug.shadow_map_size, 256..=4096).text("Shadow map size")).changed(),
        SettingId::ShadowCascades => ui.add(egui::Slider::new(&mut render_debug.shadow_cascades, 1..=4).text("Shadow cascades")).changed(),
        SettingId::ShadowMaxDistance => ui.add(egui::Slider::new(&mut render_debug.shadow_max_distance, 16.0..=320.0).text("Shadow max distance")).changed(),
        SettingId::ShadowOpacity => ui.add(egui::Slider::new(&mut render_debug.shadow_opacity, 0.0..=1.0).text("Shadow opacity")).changed(),
        SettingId::PlayerShadowOpacity => ui.add(egui::Slider::new(&mut render_debug.player_shadow_opacity, 0.0..=1.0).text("Player shadow opacity")).changed(),
        SettingId::WaterReflectionsEnabled => ui.checkbox(&mut render_debug.water_reflections_enabled, "Water reflections").changed(),
        SettingId::WaterReflectionStrength => ui.add(egui::Slider::new(&mut render_debug.water_reflection_strength, 0.0..=3.0).text("Water reflection strength")).changed(),
        SettingId::WaterReflectionNearBoost => ui.add(egui::Slider::new(&mut render_debug.water_reflection_near_boost, 0.0..=1.0).text("Near reflection boost")).changed(),
        SettingId::WaterReflectionBlueTint => ui.checkbox(&mut render_debug.water_reflection_blue_tint, "Blue reflection tint").changed(),
        SettingId::WaterReflectionTintStrength => ui.add(egui::Slider::new(&mut render_debug.water_reflection_tint_strength, 0.0..=2.0).text("Blue tint strength")).changed(),
        SettingId::WaterWaveStrength => ui.add(egui::Slider::new(&mut render_debug.water_wave_strength, 0.0..=1.2).text("Water wave strength")).changed(),
        SettingId::WaterWaveSpeed => ui.add(egui::Slider::new(&mut render_debug.water_wave_speed, 0.0..=3.0).text("Water wave speed")).changed(),
        SettingId::WaterWaveDetailStrength => ui.add(egui::Slider::new(&mut render_debug.water_wave_detail_strength, 0.0..=1.0).text("Water detail wave strength")).changed(),
        SettingId::WaterWaveDetailScale => ui.add(egui::Slider::new(&mut render_debug.water_wave_detail_scale, 1.0..=8.0).text("Water detail wave scale")).changed(),
        SettingId::WaterWaveDetailSpeed => ui.add(egui::Slider::new(&mut render_debug.water_wave_detail_speed, 0.0..=4.0).text("Water detail wave speed")).changed(),
        SettingId::WaterReflectionEdgeFade => ui.add(egui::Slider::new(&mut render_debug.water_reflection_edge_fade, 0.01..=0.5).text("Reflection edge fade")).changed(),
        SettingId::WaterReflectionSkyFill => ui.add(egui::Slider::new(&mut render_debug.water_reflection_sky_fill, 0.0..=1.0).text("Reflection sky fallback")).changed(),
        SettingId::WaterReflectionScreenSpace => ui.checkbox(&mut render_debug.water_reflection_screen_space, "SSR reflections").changed(),
        SettingId::WaterSsrSteps => ui.add(egui::Slider::new(&mut render_debug.water_ssr_steps, 4..=64).text("SSR ray steps")).changed(),
        SettingId::WaterSsrThickness => ui.add(egui::Slider::new(&mut render_debug.water_ssr_thickness, 0.02..=2.0).text("SSR hit thickness")).changed(),
        SettingId::WaterSsrMaxDistance => ui.add(egui::Slider::new(&mut render_debug.water_ssr_max_distance, 4.0..=400.0).text("SSR max distance")).changed(),
        SettingId::WaterSsrStride => ui.add(egui::Slider::new(&mut render_debug.water_ssr_stride, 0.2..=8.0).text("SSR step stride")).changed(),
        SettingId::SoundMaster => volume_slider(ui, &mut sound_settings.master, "Master volume"),
        SettingId::SoundMusic => volume_slider(ui, &mut sound_settings.music, "Music volume"),
        SettingId::SoundRecord => volume_slider(ui, &mut sound_settings.record, "Record volume"),
        SettingId::SoundWeather => volume_slider(ui, &mut sound_settings.weather, "Weather volume"),
        SettingId::SoundBlock => volume_slider(ui, &mut sound_settings.block, "Block volume"),
        SettingId::SoundHostile => volume_slider(ui, &mut sound_settings.hostile, "Hostile volume"),
        SettingId::SoundNeutral => volume_slider(ui, &mut sound_settings.neutral, "Neutral volume"),
        SettingId::SoundPlayer => volume_slider(ui, &mut sound_settings.player, "Player volume"),
        SettingId::SoundAmbient => volume_slider(ui, &mut sound_settings.ambient, "Ambient volume"),
        SettingId::ChatBackgroundOpacity => ui.add(egui::Slider::new(&mut state.chat_background_opacity, 0.0..=255.0).text("Chat background opacity")).changed(),
        SettingId::ChatFontSize => ui.add(egui::Slider::new(&mut state.chat_font_size, 10.0..=28.0).text("Chat font size")).changed(),
        SettingId::ScoreboardBackgroundOpacity => ui.add(egui::Slider::new(&mut state.scoreboard_background_opacity, 0.0..=255.0).text("Scoreboard background opacity")).changed(),
        SettingId::ScoreboardFontSize => ui.add(egui::Slider::new(&mut state.scoreboard_font_size, 10.0..=28.0).text("Scoreboard font size")).changed(),
        SettingId::TitleBackgroundOpacity => ui.add(egui::Slider::new(&mut state.title_background_opacity, 0.0..=255.0).text("Title background opacity")).changed(),
        SettingId::TitleFontSize => ui.add(egui::Slider::new(&mut state.title_font_size, 14.0..=56.0).text("Title font size")).changed(),
        SettingId::ShaderDebugView => {
            let mut selected = render_debug.cutout_debug_mode;
            egui::ComboBox::from_label("Shader debug view")
                .selected_text(match selected {
                    1 => "Pass id",
                    2 => "Atlas rgb",
                    3 => "Atlas alpha",
                    4 => "Vertex tint",
                    5 => "Linear depth",
                    6 => "Pass flags",
                    7 => "Alpha + pass",
                    8 => "Cutout lit flags",
                    _ => "Off",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Off");
                    ui.selectable_value(&mut selected, 1, "Pass id");
                    ui.selectable_value(&mut selected, 2, "Atlas rgb");
                    ui.selectable_value(&mut selected, 3, "Atlas alpha");
                    ui.selectable_value(&mut selected, 4, "Vertex tint");
                    ui.selectable_value(&mut selected, 5, "Linear depth");
                    ui.selectable_value(&mut selected, 6, "Pass flags");
                    ui.selectable_value(&mut selected, 7, "Alpha + pass");
                    ui.selectable_value(&mut selected, 8, "Cutout lit flags");
                });
            if selected != render_debug.cutout_debug_mode {
                render_debug.cutout_debug_mode = selected;
                true
            } else {
                false
            }
        }
        SettingId::FrustumFovDebug => ui.checkbox(&mut render_debug.frustum_fov_debug, "Frustum FOV debug").changed(),
        SettingId::FrustumFov => ui.add(egui::Slider::new(&mut render_debug.frustum_fov_deg, 30.0..=140.0).text("Frustum FOV")).changed(),
        SettingId::ShowChunkBorders => ui.checkbox(&mut render_debug.show_chunk_borders, "Show chunk borders").changed(),
        SettingId::MeshJobsPerFrame => ui.add(egui::Slider::new(&mut render_debug.mesh_enqueue_budget_per_frame, 1..=128).text("Mesh jobs per frame")).changed(),
        SettingId::MeshUploadsPerFrame => ui.add(egui::Slider::new(&mut render_debug.mesh_apply_budget_per_frame, 1..=64).text("Mesh uploads per frame")).changed(),
        SettingId::MaxAsyncMeshing => ui.add(egui::Slider::new(&mut render_debug.mesh_max_in_flight, 1..=256).text("Max async meshing")).changed(),
    }
}

fn render_category_extras(
    category: SettingsCategory,
    ui: &mut egui::Ui,
    state: &mut ConnectUiState,
    render_debug: &mut RenderDebugSettings,
    sound_settings: &mut SoundSettings,
    primary_window: &mut Option<bevy::ecs::change_detection::Mut<'_, Window>>,
) {
    match category {
        SettingsCategory::Diagnostics => {
            ui.label("Entity hitboxes toggle: H");
            ui.add_space(8.0);
            if ui.button("Force remesh chunks").clicked() {
                render_debug.force_remesh = true;
            }
            if ui.button("Clear + regenerate all chunk meshes").clicked() {
                render_debug.clear_and_rebuild_meshes = true;
            }
            if ui.button("Rebuild render materials").clicked() {
                render_debug.material_rebuild_nonce =
                    render_debug.material_rebuild_nonce.wrapping_add(1);
            }
        }
        SettingsCategory::System => {
            if ui.button("Reset All Settings To Default").clicked() {
                *render_debug = RenderDebugSettings::default();
                *sound_settings = SoundSettings::default();
                state.vsync_enabled = false;
                if let Some(window) = primary_window.as_deref_mut() {
                    window.present_mode = PresentMode::AutoNoVsync;
                }
                match save_client_options(&state.options_path, state, render_debug, sound_settings) {
                    Ok(()) => {
                        state.options_status = format!("Saved {}", state.options_path);
                        state.options_dirty = false;
                    }
                    Err(err) => state.options_status = err,
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Options file");
                ui.text_edit_singleline(&mut state.options_path);
            });
            ui.horizontal(|ui| {
                if ui.button("Load").clicked() {
                    let options_path = state.options_path.clone();
                    if let Some(window) = primary_window.as_deref_mut() {
                        match load_client_options(
                            &options_path,
                            state,
                            render_debug,
                            sound_settings,
                            window,
                        ) {
                            Ok(()) => {
                                state.options_status = format!("Loaded {}", options_path);
                            }
                            Err(err) => state.options_status = err,
                        }
                    } else {
                        state.options_status =
                            "Unable to load options: primary window unavailable".to_string();
                    }
                }
                if ui.button("Save").clicked() {
                    match save_client_options(
                        &state.options_path,
                        state,
                        render_debug,
                        sound_settings,
                    ) {
                        Ok(()) => {
                            state.options_status = format!("Saved {}", state.options_path);
                            state.options_dirty = false;
                        }
                        Err(err) => state.options_status = err,
                    }
                }
            });
            if !state.options_status.is_empty() {
                ui.label(&state.options_status);
            }
        }
        _ => {}
    }
}

fn volume_slider(ui: &mut egui::Ui, value: &mut f32, label: &str) -> bool {
    ui.add(
        egui::Slider::new(value, 0.0..=1.0)
            .text(label)
            .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
    )
    .changed()
}
