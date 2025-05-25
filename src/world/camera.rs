use bevy::prelude::{Component, Resource};

#[derive(Component)]
pub struct MainCamera;

#[derive(Debug, Resource)]
pub struct CameraSettings {
    pub pitch_sensitivity: f32,
    pub yaw_sensitivity: f32,
    pub fov: f32,
    pub movement_speed: f32,
}
impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            pitch_sensitivity: 0.75,
            yaw_sensitivity: 0.75,
            fov: 90.0,
            movement_speed: 15.0
        }
    }
}