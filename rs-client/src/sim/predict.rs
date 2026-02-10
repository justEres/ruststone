use super::types::{InputState, PlayerSimState, PredictedFrame};

#[derive(Debug)]
pub struct PredictionBuffer {
    capacity: usize,
    frames: Vec<PredictedFrame>,
    valid: Vec<bool>,
    latest_tick: Option<u32>,
}

impl PredictionBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            frames: vec![
                PredictedFrame {
                    tick: 0,
                    input: InputState::default(),
                    state: PlayerSimState::default(),
                };
                capacity
            ],
            valid: vec![false; capacity],
            latest_tick: None,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn latest_tick(&self) -> Option<u32> {
        self.latest_tick
    }

    pub fn push(&mut self, frame: PredictedFrame) {
        let idx = (frame.tick as usize) % self.capacity;
        self.frames[idx] = frame;
        self.valid[idx] = true;
        self.latest_tick = Some(frame.tick);
    }

    pub fn get_by_tick(&self, tick: u32) -> Option<&PredictedFrame> {
        let idx = (tick as usize) % self.capacity;
        if !self.valid[idx] {
            return None;
        }
        let frame = &self.frames[idx];
        if frame.tick != tick {
            return None;
        }
        Some(frame)
    }

    pub fn get_by_tick_mut(&mut self, tick: u32) -> Option<&mut PredictedFrame> {
        let idx = (tick as usize) % self.capacity;
        if !self.valid[idx] {
            return None;
        }
        let frame = &mut self.frames[idx];
        if frame.tick != tick {
            return None;
        }
        Some(frame)
    }

    pub fn truncate_older_than(&mut self, tick_min: u32) {
        for i in 0..self.capacity {
            if self.valid[i] && self.frames[i].tick < tick_min {
                self.valid[i] = false;
            }
        }
    }
}
