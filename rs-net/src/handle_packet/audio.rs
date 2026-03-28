use super::*;

pub(super) fn handle_packet(pkt: Packet, to_main: &crossbeam::channel::Sender<FromNetMessage>) {
    match pkt {
        Packet::NamedSoundEffect(sound) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: sound.name,
                position: packet_sound_position(sound.x, sound.y, sound.z),
                volume: sound.volume,
                pitch: sound.pitch,
                category_override: SoundCategory::from_vanilla_id(sound.category.0),
                distance_delay: false,
            }));
        }
        Packet::NamedSoundEffect_u8(sound) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: sound.name,
                position: packet_sound_position(sound.x, sound.y, sound.z),
                volume: sound.volume,
                pitch: f32::from(sound.pitch) / 63.0,
                category_override: SoundCategory::from_vanilla_id(sound.category.0),
                distance_delay: false,
            }));
        }
        Packet::NamedSoundEffect_u8_NoCategory(sound) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: sound.name,
                position: packet_sound_position(sound.x, sound.y, sound.z),
                volume: sound.volume,
                pitch: f32::from(sound.pitch) / 63.0,
                category_override: None,
                distance_delay: false,
            }));
        }
        Packet::Effect(effect) => {
            send_aux_sound_effect(
                to_main,
                effect.effect_id,
                bevy::prelude::Vec3::new(
                    effect.location.x as f32 + 0.5,
                    effect.location.y as f32 + 0.5,
                    effect.location.z as f32 + 0.5,
                ),
                effect.data,
            );
        }
        Packet::Effect_u8y(effect) => {
            send_aux_sound_effect(
                to_main,
                effect.effect_id,
                bevy::prelude::Vec3::new(
                    effect.x as f32 + 0.5,
                    f32::from(effect.y) + 0.5,
                    effect.z as f32 + 0.5,
                ),
                effect.data,
            );
        }
        Packet::Explosion(explosion) => {
            let _ = to_main.send(FromNetMessage::Sound(SoundEvent::World {
                event_id: "minecraft:random.explode".to_string(),
                position: bevy::prelude::Vec3::new(explosion.x, explosion.y, explosion.z),
                volume: 4.0,
                pitch: 1.0,
                category_override: Some(SoundCategory::Block),
                distance_delay: false,
            }));
        }
        _ => {}
    }
}
