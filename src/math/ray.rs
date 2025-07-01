use crate::math::block::Vec3Ext;
use bevy::prelude::{IVec3, Vec3};
use crate::world::block::Direction;

/// Performs a raycast from a starting position to a direction in the world.
/// The test function is run every block iteration, returning true on a ray hit and false on a ray miss.
/// If the test function returns an error, the raycast is abrupted immediately and the error is returned.
pub fn block_raycast(
    start: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut test_function: impl FnMut(&RayContext, Vec3, Direction, IVec3) -> Result<bool, Box<dyn std::error::Error>>,
) -> Result<RayResult, Box<dyn std::error::Error>> {


    // println!("Raycasting from {} in direction {}", start, direction);



    // prevents division by zero
    let mut direction = direction.normalize_or_zero();
    if direction.x == 0.0 {
        // prevent division by zero issues
        direction.x = 0.000000001;
    }
    if direction.y == 0.0 {
        direction.y = 0.000000001;
    }
    if direction.z == 0.0 {
        direction.z = 0.000000001;
    }

    // following distances are based on the formula p + t * d, where p is the origin, d is the dir vector, and t is the amount

    let context = RayContext {
        start,
        direction,
    };


    // the step vectors. the signs tell you which way to step
    let step = direction.signum();

    // direction = opposite of the direction step is going
    let x_face = if step.x > 0.0 {
        Direction::West
    } else {
        Direction::East
    };

    let y_face = if step.y > 0.0 {
        Direction::Down
    } else {
        Direction::Up
    };
    let z_face = if step.z > 0.0 {
        Direction::South
    } else {
        Direction::North
    };




    // println!("Step vec: {}", step);

    // the delta vector, i.e. delta_t.x * direction will have an x length of 1
    let delta_t = 1.0 / direction.abs();

    // Get current voxel position
    let mut grid_pos = start.floor();

    // max distance to travel to reach the next grid line.
    // let mut max_t = (grid_pos + step - start) / direction;
    let mut max_t = ( ((step + 1.0) / 2.0) + (grid_pos - start) ) / direction;

    // keep track of how
    let mut traveled_distance = 0.0;

    while traveled_distance < max_distance {
        let axis = argmin(max_t);

        let face = match axis {
            0 => x_face,
            1 => y_face,
            2 => z_face,
            _ => panic!("Dead branch")
        };



        grid_pos[axis] += step[axis];

        let is_hit = test_function(&context, start + (max_t[axis] * direction), face, grid_pos.as_block_pos())?;
        if is_hit {
            return Ok(RayResult::Hit(start + (max_t[axis] * direction), face, grid_pos.as_block_pos()))
        }
        traveled_distance = max_t[axis];
        // println!("Distance traveled: {}", traveled_distance);
        max_t[axis] += delta_t[axis];
    }

    Ok(RayResult::Miss)
}

// gets the minimum value, returns 0 1 or 2 for x y and z respectively.
fn argmin(vec: Vec3) -> usize {
    let mut min = vec.x;
    let mut index = 0;
    if vec.y < min {
        min = vec.y;
        index = 1;
    }
    if vec.z < min {
        index = 2;
    }
    index
}

/// The result of a raycast. 
/// Either a hit containing the Vec3 representing the point 
/// on the block the ray intersected and block pos of the raycast, or a miss.
#[derive(Debug, Clone)]
pub enum RayResult {
    Hit(Vec3, Direction, IVec3),
    Miss
}

/// The context of a Raycast, for use in test functions.
#[derive(Debug, Clone)]
pub struct RayContext {
    pub start: Vec3,
    pub direction: Vec3,
}