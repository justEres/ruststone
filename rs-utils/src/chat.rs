use std::collections::VecDeque;

use bevy::ecs::resource::Resource;

use crate::world::TitleTimes;

#[derive(Resource, Default)]
pub struct Chat(pub VecDeque<String>, pub String);

pub enum TitleMessage {
    SetTitle {
        text: String,
    },
    SetSubtitle {
        text: String,
    },
    SetActionBar {
        text: String,
    },
    SetTimes {
        fade_in_ticks: i32,
        stay_ticks: i32,
        fade_out_ticks: i32,
    },
    Clear,
    Reset,
}

impl TitleMessage {
    pub fn times(fade_in_ticks: i32, stay_ticks: i32, fade_out_ticks: i32) -> Self {
        Self::SetTimes {
            fade_in_ticks,
            stay_ticks,
            fade_out_ticks,
        }
    }

    pub fn as_times(&self) -> Option<TitleTimes> {
        match self {
            Self::SetTimes {
                fade_in_ticks,
                stay_ticks,
                fade_out_ticks,
            } => Some(TitleTimes {
                fade_in_ticks: *fade_in_ticks,
                stay_ticks: *stay_ticks,
                fade_out_ticks: *fade_out_ticks,
            }),
            _ => None,
        }
    }
}
