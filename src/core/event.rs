use bevy::prelude::{Event, Vec3};

#[derive(Event)]
pub struct PlayerMovedEvent {
    pub old: Vec3,
    pub new: Vec3
}