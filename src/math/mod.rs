//! Module that provides helpful math functions.

use bevy::math::{Vec2, Vec3};
use noiz::NoiseFunction;

pub mod ray;
pub mod block;
pub mod noise;

/// Trait alias for a 2D Noise function. See [`NoiseFunction`] for more info
pub trait NoiseFunction2D: NoiseFunction<Vec2, Output=f32> {}
impl <T: NoiseFunction<Vec2, Output=f32>> NoiseFunction2D for T {}



/// Trait alias for a 3D Noise function. See [`NoiseFunction`] for more info
pub trait NoiseFunction3D: NoiseFunction<Vec3, Output=f32> {}
impl <T: NoiseFunction<Vec3, Output=f32>> NoiseFunction3D for T {}