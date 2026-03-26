use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, SpatialAudioSink,
    SpatialListener, Volume,
};
use bevy::prelude::*;
use rand::distributions::{Distribution, WeightedIndex};
use rand::thread_rng;
use rs_entity::{RemoteEntity, RemoteEntityRegistry};
use rs_render::{Player, PlayerCamera};
use rs_utils::{
    SoundCategory, SoundEvent, SoundEventQueue, SoundSettings, sound_cache_minecraft_root,
    texturepack_minecraft_root,
};
use serde::Deserialize;
use tracing::warn;

use crate::events::PlayingSound;
use crate::{MAX_PITCH, MIN_PITCH};

#[derive(Resource, Default)]
pub(crate) struct SoundRegistry {
    events: HashMap<String, SoundEventDefinition>,
}

#[derive(Clone, Debug)]
pub(crate) struct SoundEventDefinition {
    category: SoundCategory,
    sounds: Vec<SoundEntry>,
}

#[derive(Clone, Debug)]
enum SoundEntry {
    File(SoundFile),
    EventRef(String),
}

#[derive(Clone, Debug)]
struct SoundFile {
    resource_path: String,
    weight: u32,
    volume: f32,
    pitch: f32,
    stream: bool,
}

#[derive(Resource, Default)]
pub(crate) struct RuntimeAudioAssets {
    loaded: HashMap<String, Handle<AudioSource>>,
}

#[derive(Resource)]
pub(crate) struct SoundAssetResolver {
    sources: Vec<AssetSource>,
    cache_root: PathBuf,
    warned_missing_events: HashSet<String>,
    warned_missing_assets: HashSet<String>,
}

#[derive(Clone)]
enum AssetSource {
    Direct { minecraft_root: PathBuf },
    Indexed(IndexedAssetSource),
}

#[derive(Clone)]
struct IndexedAssetSource {
    objects_root: PathBuf,
    objects: HashMap<String, IndexedObject>,
}

#[derive(Clone, Debug, Deserialize)]
struct IndexedObject {
    hash: String,
}

pub(crate) fn setup_sound_runtime(mut commands: Commands) {
    let mut resolver = SoundAssetResolver::discover();
    let registry = resolver.load_registry();
    commands.insert_resource(resolver);
    commands.insert_resource(registry);
    commands.insert_resource(RuntimeAudioAssets::default());
}

pub(crate) fn ensure_spatial_listener(
    mut commands: Commands,
    query: Query<Entity, (With<PlayerCamera>, Without<SpatialListener>)>,
) {
    for entity in &query {
        commands.entity(entity).insert(SpatialListener::new(0.35));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn drain_sound_events(
    mut commands: Commands,
    mut queue: ResMut<SoundEventQueue>,
    settings: Res<SoundSettings>,
    registry: Res<SoundRegistry>,
    mut resolver: ResMut<SoundAssetResolver>,
    mut audio_assets: ResMut<Assets<AudioSource>>,
    mut runtime_assets: ResMut<RuntimeAudioAssets>,
    remote_registry: Res<RemoteEntityRegistry>,
    remote_entities: Query<(&GlobalTransform, &RemoteEntity)>,
    player_query: Query<&GlobalTransform, With<Player>>,
    sink_query: Query<(
        Entity,
        &PlayingSound,
        Option<&AudioSink>,
        Option<&SpatialAudioSink>,
    )>,
) {
    let events = queue.drain();
    if events.is_empty() {
        return;
    }

    for event in events {
        match event {
            SoundEvent::Ui {
                event_id,
                volume,
                pitch,
                category_override,
            } => {
                let Some((handle, category, base_gain, final_gain, final_pitch, stream)) =
                    prepare_sound(
                        &event_id,
                        category_override,
                        volume,
                        pitch,
                        &settings,
                        &registry,
                        &mut resolver,
                        &mut runtime_assets,
                        &mut audio_assets,
                    )
                else {
                    continue;
                };
                let mut playback = PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(final_gain))
                    .with_speed(final_pitch);
                playback.spatial = false;
                spawn_sound_entity(
                    &mut commands,
                    handle,
                    playback,
                    None,
                    category,
                    base_gain,
                    stream,
                );
            }
            SoundEvent::World {
                event_id,
                position,
                volume,
                pitch,
                category_override,
                distance_delay: _,
            } => {
                let Some((handle, category, base_gain, final_gain, final_pitch, stream)) =
                    prepare_sound(
                        &event_id,
                        category_override,
                        volume,
                        pitch,
                        &settings,
                        &registry,
                        &mut resolver,
                        &mut runtime_assets,
                        &mut audio_assets,
                    )
                else {
                    continue;
                };
                let mut playback = PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(final_gain))
                    .with_speed(final_pitch);
                playback.spatial = true;
                spawn_sound_entity(
                    &mut commands,
                    handle,
                    playback,
                    Some(position),
                    category,
                    base_gain,
                    stream,
                );
            }
            SoundEvent::Entity {
                event_id,
                entity_id,
                volume,
                pitch,
                category_override,
            } => {
                let resolved_position = if remote_registry.local_entity_id == Some(entity_id) {
                    player_query.iter().next().map(|gt| gt.translation())
                } else {
                    remote_entities.iter().find_map(|(gt, remote)| {
                        (remote.server_id == entity_id).then_some(gt.translation())
                    })
                };
                let Some(position) = resolved_position else {
                    continue;
                };
                let Some((handle, category, base_gain, final_gain, final_pitch, stream)) =
                    prepare_sound(
                        &event_id,
                        category_override,
                        volume,
                        pitch,
                        &settings,
                        &registry,
                        &mut resolver,
                        &mut runtime_assets,
                        &mut audio_assets,
                    )
                else {
                    continue;
                };
                let mut playback = PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(final_gain))
                    .with_speed(final_pitch);
                playback.spatial = true;
                spawn_sound_entity(
                    &mut commands,
                    handle,
                    playback,
                    Some(position),
                    category,
                    base_gain,
                    stream,
                );
            }
            SoundEvent::Stop { scope } => {
                for (entity, playing, audio_sink, spatial_sink) in &sink_query {
                    let should_stop = match scope {
                        rs_utils::SoundStopScope::All => true,
                        rs_utils::SoundStopScope::Category(category) => {
                            playing.category == category
                        }
                    };
                    if !should_stop {
                        continue;
                    }
                    if let Some(sink) = audio_sink {
                        sink.stop();
                    }
                    if let Some(sink) = spatial_sink {
                        sink.stop();
                    }
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

pub(crate) fn sync_playing_sound_volumes(
    settings: Res<SoundSettings>,
    mut audio_sinks: Query<(&PlayingSound, &mut AudioSink)>,
    mut spatial_sinks: Query<(&PlayingSound, &mut SpatialAudioSink)>,
) {
    if !settings.is_changed() {
        return;
    }

    for (playing, mut sink) in &mut audio_sinks {
        sink.set_volume(Volume::Linear(
            settings.final_gain(playing.category, playing.base_gain),
        ));
    }
    for (playing, mut sink) in &mut spatial_sinks {
        sink.set_volume(Volume::Linear(
            settings.final_gain(playing.category, playing.base_gain),
        ));
    }
}

fn spawn_sound_entity(
    commands: &mut Commands,
    handle: Handle<AudioSource>,
    playback: PlaybackSettings,
    position: Option<Vec3>,
    category: SoundCategory,
    base_gain: f32,
    _stream: bool,
) {
    let mut entity = commands.spawn((
        AudioPlayer::new(handle),
        playback,
        PlayingSound {
            category,
            base_gain,
        },
    ));
    if let Some(position) = position {
        entity.insert((
            Transform::from_translation(position),
            GlobalTransform::default(),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_sound(
    event_id: &str,
    category_override: Option<SoundCategory>,
    event_volume: f32,
    event_pitch: f32,
    settings: &SoundSettings,
    registry: &SoundRegistry,
    resolver: &mut SoundAssetResolver,
    runtime_assets: &mut RuntimeAudioAssets,
    audio_assets: &mut Assets<AudioSource>,
) -> Option<(Handle<AudioSource>, SoundCategory, f32, f32, f32, bool)> {
    let normalized = normalize_event_id(event_id);
    let Some((category, file)) = registry.pick_sound(&normalized, resolver) else {
        resolver.warn_missing_event(normalized.as_str());
        return None;
    };
    let resolved_category = category_override.unwrap_or(category);
    let bytes = resolver.read_sound_file(file.resource_path.as_str())?;
    let handle = if let Some(existing) = runtime_assets.loaded.get(&file.resource_path) {
        existing.clone()
    } else {
        let handle = audio_assets.add(AudioSource {
            bytes: bytes.into_boxed_slice().into(),
        });
        runtime_assets
            .loaded
            .insert(file.resource_path.clone(), handle.clone());
        handle
    };
    let unclamped_gain = event_volume.max(0.0) * file.volume.max(0.0);
    let final_gain = settings.final_gain(resolved_category, unclamped_gain);
    let final_pitch = (event_pitch * file.pitch).clamp(MIN_PITCH, MAX_PITCH);
    Some((
        handle,
        resolved_category,
        unclamped_gain,
        final_gain,
        final_pitch,
        file.stream,
    ))
}

impl SoundRegistry {
    fn pick_sound(
        &self,
        event_id: &str,
        resolver: &mut SoundAssetResolver,
    ) -> Option<(SoundCategory, SoundFile)> {
        self.pick_sound_inner(event_id, resolver, 0)
    }

    fn pick_sound_inner(
        &self,
        event_id: &str,
        resolver: &mut SoundAssetResolver,
        depth: usize,
    ) -> Option<(SoundCategory, SoundFile)> {
        if depth > 8 {
            return None;
        }
        let def = self.events.get(event_id)?;
        let mut file_candidates = Vec::new();
        let mut weights = Vec::new();
        for entry in &def.sounds {
            match entry {
                SoundEntry::File(file) => {
                    file_candidates.push(file.clone());
                    weights.push(file.weight.max(1));
                }
                SoundEntry::EventRef(other) => {
                    if let Some((_, file)) = self.pick_sound_inner(other, resolver, depth + 1) {
                        weights.push(file.weight.max(1));
                        file_candidates.push(file);
                    }
                }
            }
        }
        if file_candidates.is_empty() {
            resolver.warn_missing_event(event_id);
            return None;
        }
        let idx = WeightedIndex::new(weights)
            .ok()
            .map(|dist| dist.sample(&mut thread_rng()))
            .unwrap_or(0);
        Some((def.category, file_candidates[idx].clone()))
    }
}

impl SoundAssetResolver {
    fn discover() -> Self {
        let mut sources = Vec::new();
        let pack_root = texturepack_minecraft_root();
        if pack_root.exists() {
            sources.push(AssetSource::Direct {
                minecraft_root: pack_root,
            });
        }

        let cache_root = sound_cache_minecraft_root();
        if cache_root.exists() {
            sources.push(AssetSource::Direct {
                minecraft_root: cache_root.clone(),
            });
        }

        if let Some(source) = IndexedAssetSource::from_parts(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../MavenMCP-1.8.9/test_run/assets/indexes/1.8.json"),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../MavenMCP-1.8.9/test_run/assets/objects"),
        ) {
            sources.push(AssetSource::Indexed(source));
        }

        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            if let Some(source) = IndexedAssetSource::from_parts(
                home.join(".minecraft/assets/indexes/1.8.json"),
                home.join(".minecraft/assets/objects"),
            ) {
                sources.push(AssetSource::Indexed(source));
            }
        }

        Self {
            sources,
            cache_root,
            warned_missing_events: HashSet::new(),
            warned_missing_assets: HashSet::new(),
        }
    }

    fn load_registry(&mut self) -> SoundRegistry {
        for source in &self.sources {
            if let Some(bytes) = source.read_relative("sounds.json") {
                self.ensure_cache("sounds.json", &bytes);
                match serde_json::from_slice::<HashMap<String, RawSoundDefinition>>(&bytes) {
                    Ok(raw) => return SoundRegistry::from_raw(raw),
                    Err(err) => {
                        warn!("Failed to parse sounds.json: {}", err);
                        break;
                    }
                }
            }
        }
        warn!("No sounds.json found; sound playback will stay inactive");
        SoundRegistry::default()
    }

    fn read_sound_file(&mut self, resource_path: &str) -> Option<Vec<u8>> {
        let relative = resource_path
            .strip_prefix("minecraft:")
            .unwrap_or(resource_path)
            .trim_start_matches('/');
        let relative = relative
            .strip_prefix("sounds/")
            .map(|path| format!("sounds/{path}"))
            .unwrap_or_else(|| relative.to_string());
        for source in &self.sources {
            if let Some(bytes) = source.read_relative(relative.as_str()) {
                self.ensure_cache(relative.as_str(), &bytes);
                return Some(bytes);
            }
        }
        self.warn_missing_asset(relative.as_str());
        None
    }

    fn ensure_cache(&self, relative: &str, bytes: &[u8]) {
        let out_path = self.cache_root.join(relative);
        if out_path.exists() {
            return;
        }
        if let Some(parent) = out_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(out_path, bytes);
    }

    fn warn_missing_event(&mut self, event_id: &str) {
        if self.warned_missing_events.insert(event_id.to_string()) {
            warn!("Missing sound event: {}", event_id);
        }
    }

    fn warn_missing_asset(&mut self, asset: &str) {
        if self.warned_missing_assets.insert(asset.to_string()) {
            warn!("Missing sound asset: {}", asset);
        }
    }
}

impl AssetSource {
    fn read_relative(&self, relative: &str) -> Option<Vec<u8>> {
        match self {
            AssetSource::Direct { minecraft_root } => fs::read(minecraft_root.join(relative)).ok(),
            AssetSource::Indexed(source) => source.read_relative(relative),
        }
    }
}

impl IndexedAssetSource {
    fn from_parts(index_path: PathBuf, objects_root: PathBuf) -> Option<Self> {
        if !index_path.exists() || !objects_root.exists() {
            return None;
        }
        let bytes = fs::read(&index_path).ok()?;
        let parsed = serde_json::from_slice::<RawAssetIndex>(&bytes).ok()?;
        Some(Self {
            objects_root,
            objects: parsed.objects,
        })
    }

    fn read_relative(&self, relative: &str) -> Option<Vec<u8>> {
        let key = format!("minecraft/{}", relative.trim_start_matches('/'));
        let object = self.objects.get(&key)?;
        let hash_prefix = &object.hash[..2];
        let path = self
            .objects_root
            .join(hash_prefix)
            .join(object.hash.as_str());
        fs::read(path).ok()
    }
}

#[derive(Deserialize)]
struct RawAssetIndex {
    objects: HashMap<String, IndexedObject>,
}

#[derive(Deserialize)]
struct RawSoundDefinition {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    sounds: Vec<RawSoundEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawSoundEntry {
    Simple(String),
    Detailed(RawSoundEntryDetailed),
}

#[derive(Deserialize)]
struct RawSoundEntryDetailed {
    name: String,
    #[serde(default)]
    volume: Option<f32>,
    #[serde(default)]
    pitch: Option<f32>,
    #[serde(default)]
    weight: Option<u32>,
    #[serde(default)]
    stream: bool,
    #[serde(default, rename = "type")]
    entry_type: Option<String>,
}

impl SoundRegistry {
    fn from_raw(raw: HashMap<String, RawSoundDefinition>) -> Self {
        let mut events = HashMap::new();
        for (event_name, raw_def) in raw {
            let category = raw_def
                .category
                .as_deref()
                .and_then(parse_sound_category_name)
                .unwrap_or(SoundCategory::Master);
            let mut sounds = Vec::new();
            for entry in raw_def.sounds {
                match entry {
                    RawSoundEntry::Simple(name) => {
                        let normalized_name = normalize_sound_entry_name("minecraft", &name);
                        if normalized_name.ends_with(".ogg") {
                            sounds.push(SoundEntry::File(SoundFile {
                                resource_path: normalized_name,
                                weight: 1,
                                volume: 1.0,
                                pitch: 1.0,
                                stream: false,
                            }));
                        } else {
                            sounds.push(SoundEntry::EventRef(normalize_event_id(&normalized_name)));
                        }
                    }
                    RawSoundEntry::Detailed(entry) => {
                        let kind = entry.entry_type.as_deref().unwrap_or("file");
                        let normalized_name = normalize_sound_entry_name("minecraft", &entry.name);
                        if kind == "event" || !normalized_name.ends_with(".ogg") {
                            sounds.push(SoundEntry::EventRef(normalize_event_id(&normalized_name)));
                        } else {
                            sounds.push(SoundEntry::File(SoundFile {
                                resource_path: normalized_name,
                                weight: entry.weight.unwrap_or(1),
                                volume: entry.volume.unwrap_or(1.0),
                                pitch: entry.pitch.unwrap_or(1.0),
                                stream: entry.stream,
                            }));
                        }
                    }
                }
            }
            events.insert(
                normalize_event_id(&event_name),
                SoundEventDefinition { category, sounds },
            );
        }
        Self { events }
    }
}

fn parse_sound_category_name(name: &str) -> Option<SoundCategory> {
    match name {
        "master" => Some(SoundCategory::Master),
        "music" => Some(SoundCategory::Music),
        "record" => Some(SoundCategory::Record),
        "weather" => Some(SoundCategory::Weather),
        "block" => Some(SoundCategory::Block),
        "hostile" => Some(SoundCategory::Hostile),
        "neutral" => Some(SoundCategory::Neutral),
        "player" => Some(SoundCategory::Player),
        "ambient" => Some(SoundCategory::Ambient),
        _ => None,
    }
}

fn normalize_event_id(event_id: &str) -> String {
    if event_id.contains(':') {
        event_id.to_string()
    } else {
        format!("minecraft:{event_id}")
    }
}

fn normalize_sound_entry_name(default_namespace: &str, raw: &str) -> String {
    let namespaced = if raw.contains(':') {
        raw.to_string()
    } else {
        format!("{default_namespace}:{raw}")
    };
    let (namespace, path) = namespaced.split_once(':').unwrap_or(("minecraft", raw));
    if path.ends_with(".ogg") {
        format!("{namespace}:{path}")
    } else if path.starts_with("sounds/") {
        format!("{namespace}:{path}.ogg")
    } else {
        format!("{namespace}:sounds/{path}.ogg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vanilla_sound_registry_from_local_maven_assets() {
        let index_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../MavenMCP-1.8.9/test_run/assets/indexes/1.8.json");
        let objects_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../MavenMCP-1.8.9/test_run/assets/objects");
        let indexed = IndexedAssetSource::from_parts(index_path, objects_root).unwrap();
        let bytes = indexed.read_relative("sounds.json").unwrap();
        let raw = serde_json::from_slice::<HashMap<String, RawSoundDefinition>>(&bytes).unwrap();
        let registry = SoundRegistry::from_raw(raw);
        assert!(registry.events.contains_key("minecraft:random.click"));
    }

    #[test]
    fn resolves_sound_file_from_indexed_store() {
        let index_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../MavenMCP-1.8.9/test_run/assets/indexes/1.8.json");
        let objects_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../MavenMCP-1.8.9/test_run/assets/objects");
        let indexed = IndexedAssetSource::from_parts(index_path, objects_root).unwrap();
        let bytes = indexed.read_relative("sounds/random/click.ogg").unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn home_minecraft_index_is_optional() {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            let source = IndexedAssetSource::from_parts(
                home.join(".minecraft/assets/indexes/1.8.json"),
                home.join(".minecraft/assets/objects"),
            );
            if let Some(source) = source {
                assert!(source.read_relative("sounds.json").is_some());
            }
        }
    }
}
