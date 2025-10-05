#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    Y,
    Z,
    X,
    None,
}

impl Axis {
    pub fn as_string(&self) -> &'static str {
        match *self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
            Axis::None => "none",
        }
    }

    pub fn index(&self) -> usize {
        match *self {
            Axis::Y => 0,
            Axis::Z => 2,
            Axis::X => 1,
            Axis::None => 3,
        }
    }
}

use bevy_ecs::prelude::*;
use std::fmt;
use std::ops;

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Position {
    pub fn new(x: i32, y: i32, z: i32) -> Position {
        Position { x, y, z }
    }

    pub fn shift(self, dir: Direction) -> Position {
        let (ox, oy, oz) = dir.get_offset();
        self + (ox, oy, oz)
    }

    pub fn shift_by(self, dir: Direction, by: i32) -> Position {
        let (ox, oy, oz) = dir.get_offset();
        self + (ox * by, oy * by, oz * by)
    }
}

impl ops::Add<Position> for Position {
    type Output = Position;

    fn add(self, o: Position) -> Position {
        Position {
            x: self.x + o.x,
            y: self.y + o.y,
            z: self.z + o.z,
        }
    }
}

impl ops::Add<(i32, i32, i32)> for Position {
    type Output = Position;

    fn add(self, (x, y, z): (i32, i32, i32)) -> Position {
        Position {
            x: self.x + x,
            y: self.y + y,
            z: self.z + z,
        }
    }
}

impl ops::Sub<Position> for Position {
    type Output = Position;

    fn sub(self, o: Position) -> Position {
        Position {
            x: self.x - o.x,
            y: self.y - o.y,
            z: self.z - o.z,
        }
    }
}

impl ops::Sub<(i32, i32, i32)> for Position {
    type Output = Position;

    fn sub(self, (x, y, z): (i32, i32, i32)) -> Position {
        Position {
            x: self.x - x,
            y: self.y - y,
            z: self.z - z,
        }
    }
}

impl Default for Position {
    fn default() -> Position {
        Position::new(0, 0, 0)
    }
}

impl fmt::Debug for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<{},{},{}>", self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Invalid,
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub fn all() -> Vec<Direction> {
        vec![
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ]
    }

    pub fn from_string(val: &str) -> Direction {
        match val {
            "down" => Direction::Down,
            "up" => Direction::Up,
            "north" => Direction::North,
            "south" => Direction::South,
            "west" => Direction::West,
            "east" => Direction::East,
            _ => Direction::Invalid,
        }
    }

    pub fn opposite(&self) -> Direction {
        match *self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
            _ => unreachable!(),
        }
    }

    pub fn clockwise(&self) -> Direction {
        match *self {
            Direction::Down => Direction::Down,
            Direction::Up => Direction::Up,
            Direction::East => Direction::South,
            Direction::West => Direction::North,
            Direction::South => Direction::West,
            Direction::North => Direction::East,
            _ => unreachable!(),
        }
    }

    pub fn counter_clockwise(&self) -> Direction {
        match *self {
            Direction::Down => Direction::Down,
            Direction::Up => Direction::Up,
            Direction::East => Direction::North,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::North => Direction::West,
            _ => unreachable!(),
        }
    }

    pub fn get_offset(&self) -> (i32, i32, i32) {
        match *self {
            Direction::Down => (0, -1, 0),
            Direction::Up => (0, 1, 0),
            Direction::North => (0, 0, -1),
            Direction::South => (0, 0, 1),
            Direction::West => (-1, 0, 0),
            Direction::East => (1, 0, 0),
            _ => unreachable!(),
        }
    }

    pub fn as_string(&self) -> &'static str {
        match *self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::North => "north",
            Direction::South => "south",
            Direction::West => "west",
            Direction::East => "east",
            Direction::Invalid => "invalid",
        }
    }

    pub fn index(&self) -> usize {
        match *self {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::North => 2,
            Direction::South => 3,
            Direction::West => 4,
            Direction::East => 5,
            _ => unreachable!(),
        }
    }

    pub fn offset(&self) -> usize {
        match *self {
            Direction::North => 0,
            Direction::East => 1,
            Direction::South => 2,
            Direction::West => 3,
            Direction::Up => 4,
            Direction::Down => 5,
            _ => unreachable!(),
        }
    }

    pub fn horizontal_index(&self) -> usize {
        match *self {
            Direction::North => 2,
            Direction::South => 0,
            Direction::West => 1,
            Direction::East => 3,
            _ => unreachable!(),
        }
    }

    pub fn horizontal_offset(&self) -> usize {
        match *self {
            Direction::North => 0,
            Direction::South => 1,
            Direction::West => 2,
            Direction::East => 3,
            _ => unreachable!(),
        }
    }

    pub fn axis(&self) -> Axis {
        match *self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
            _ => unreachable!(),
        }
    }
}

/// A list of all supported versions
#[derive(PartialOrd, PartialEq, Debug, Copy, Clone)]
pub enum Version {
    Other,
    Old,
    V1_7,
    V1_8,
    V1_9,
    V1_10,
    V1_11,
    V1_12,
    V1_13,
    V1_13_2,
    V1_14,
    V1_15,
    V1_16,
    V1_16_2,
    V1_17,
    V1_18,
    V1_19,
    New,
}

impl Version {
    pub fn from_id(protocol_version: u32) -> Version {
        match protocol_version {
            0..=4 => Version::Old,
            5 => Version::V1_7,
            47 => Version::V1_8,
            107..=110 => Version::V1_9,
            210 => Version::V1_10,
            315..=316 => Version::V1_11,
            335..=340 => Version::V1_12,
            393..=401 => Version::V1_13,
            404..=404 => Version::V1_13_2,
            477..=498 => Version::V1_14,
            573..=578 => Version::V1_15,
            735..=736 => Version::V1_16,
            737..=754 => Version::V1_16_2,
            755..=756 => Version::V1_17,
            757..=758 => Version::V1_18,
            759..=760 => Version::V1_19,
            761..=u32::MAX => Version::New,
            _ => Version::Other,
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(
            self,
            Version::Old
                | Version::New
                | Version::Other
                | Version::V1_17
                | Version::V1_18
                | Version::V1_19
        )
    }
}
