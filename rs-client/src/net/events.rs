use std::collections::VecDeque;
use std::time::Instant;

use bevy::prelude::{Resource, Vec3};

#[derive(Clone, Debug)]
pub enum NetEvent {
    ServerPosLook {
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
        recv_instant: Instant,
    },
    ServerVelocity {
        velocity: Vec3,
    },
}

#[derive(Default, Resource)]
pub struct NetEventQueue {
    pub events: VecDeque<NetEvent>,
}

impl NetEventQueue {
    pub fn push(&mut self, event: NetEvent) {
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> std::collections::vec_deque::Drain<'_, NetEvent> {
        self.events.drain(..)
    }
}
