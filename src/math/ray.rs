use bevy::math::vec3;
use bevy::prelude::{IVec3, Vec3};
use crate::math::block::Vec3Ext;

pub fn block_raycast(
    start: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut test_function: impl FnMut(&RayContext, Vec3, IVec3) -> bool,
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
        grid_pos[axis] += step[axis];
        if test_function(&context, start + (max_t[axis] * direction), grid_pos.as_block_pos()) {
            return Ok(RayResult::Hit(start + (max_t[axis] * direction), grid_pos.as_block_pos()))
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

fn grid_initial(origin: Vec3, step_vec: Vec3) -> Vec3 {
    let mut ret = origin.floor();
    if step_vec.x < 0.0 {
        ret.x = origin.x.ceil();
    }
    if step_vec.y < 0.0 {
        ret.y = origin.y.ceil();
    }
    if step_vec.z < 0.0 {
        ret.z = origin.z.ceil();
    }

    ret
    // origin.floor()
}


#[derive(Debug, Clone)]
pub enum RayResult {
    Hit(Vec3, IVec3),
    Miss
}


#[derive(Debug, Clone)]
pub struct RayContext {
    start: Vec3,
    direction: Vec3,
}