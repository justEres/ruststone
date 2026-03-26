use bevy::prelude::*;
use rs_utils::{SoundCategory, SoundEvent, SoundEventQueue};

pub fn emit_ui_sound(
    queue: &mut SoundEventQueue,
    event_id: impl Into<String>,
    volume: f32,
    pitch: f32,
) {
    queue.push(SoundEvent::Ui {
        event_id: event_id.into(),
        volume,
        pitch,
        category_override: None,
    });
}

pub fn emit_world_sound(
    queue: &mut SoundEventQueue,
    event_id: impl Into<String>,
    position: Vec3,
    volume: f32,
    pitch: f32,
    category_override: Option<SoundCategory>,
) {
    queue.push(SoundEvent::World {
        event_id: event_id.into(),
        position,
        volume,
        pitch,
        category_override,
        distance_delay: false,
    });
}

pub fn emit_entity_sound(
    queue: &mut SoundEventQueue,
    event_id: impl Into<String>,
    entity_id: i32,
    volume: f32,
    pitch: f32,
    category_override: Option<SoundCategory>,
) {
    queue.push(SoundEvent::Entity {
        event_id: event_id.into(),
        entity_id,
        volume,
        pitch,
        category_override,
    });
}

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayingSound {
    pub category: SoundCategory,
    pub base_gain: f32,
}
