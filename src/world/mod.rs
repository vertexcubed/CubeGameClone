use crate::asset::block::BlockModelAsset;
use crate::core::errors::WorldError;
use crate::core::event::{JoinedWorldEvent, PlayerMovedEvent, SetBlockEvent};
use crate::core::state::MainGameState;
use crate::math::block::{BlockPos, Vec3Ext};
use crate::math::ray;
use crate::math::ray::RayResult;
use crate::registry::block::Block;
use crate::registry::{Registry, RegistryHandle};
use crate::render;
use crate::render::block::BlockTextures;
use crate::render::block::MeshDataCache;
use crate::render::material::BlockMaterial;
use crate::world::camera::{CameraSettings, MainCamera};
use crate::world::chunk::{Chunk, ChunkData, ChunkNeedsMeshing, PaletteEntry};
use crate::world::block::BlockWorld;
use bevy::asset::AssetContainer;
use bevy::color::palettes::css;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::math::bounding::{Aabb3d, IntersectsVolume};
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::*;
use bevy::render::primitives::Aabb;
use bevy::tasks::futures_lite::future;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bitvec::order::Msb0;
use bitvec::prelude::BitVec;
use bitvec::view::BitViewSized;
use rand::distr::Uniform;
use rand::Rng;
use block::{BlockState, ChunkMap, Direction};
use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use crate::world::machine::MachineWorld;

pub mod chunk;
pub mod camera;
pub mod block;
pub mod machine;

#[derive(Default)]
pub struct GameWorldPlugin;

impl Plugin for GameWorldPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraSettings>()
            // temp

            .add_systems(Update, (handle_input, look_at_block, place_and_break))
            .add_systems(PreUpdate, join_world)
            // .add_systems(Update, track_chunks_around_player)
            .add_systems(OnEnter(MainGameState::InGame), (setup_world, grab_cursor, create_world))
            .add_observer(on_set_block)
        ;
        block::add_systems(app);
    }
}

// runs once when InGame reached
fn setup_world(
    mut commands: Commands,
    camera_settings: Res<CameraSettings>,


    // mut materials: ResMut<Assets<StandardMaterial>>,
    // mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("Loading world...");
    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: camera_settings.fov.to_radians(),
            ..default()
        }),
        MainCamera,
        Transform::from_xyz(0.0, 2.0, 0.0),
        LookAtData::default(),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(25.0, 50.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y)
    ));

    // commands.spawn((
    //     LookAtData::default(),
    //     Transform::from_xyz(0.0, 0.0, 0.0),
    //     MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
    //     Mesh3d(meshes.add(Sphere {radius: 0.125}.mesh())),
    // ));

}


fn handle_input(
    mut transform: Single<&mut Transform, With<MainCamera>>,
    // mut proj: Single<&mut Projection, With<MainCamera>>,
    camera_settings: Res<CameraSettings>,
    timer: Res<Time>,
    kb_input: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut player_moved: EventWriter<PlayerMovedEvent>,
) {
    let delta = mouse_motion.delta;

    let delta_yaw = (camera_settings.yaw_sensitivity * -delta.x).to_radians();
    let delta_pitch = (camera_settings.pitch_sensitivity * -delta.y).to_radians();


    let (yaw_old, pitch_old, roll_old) = transform.rotation.to_euler(EulerRot::YXZ);

    let pitch = (pitch_old + delta_pitch).clamp(
        -89.9 * PI/180.,
        89.9 * PI/180.
    );
    let yaw = yaw_old + delta_yaw;
    let roll = roll_old;
    // important: this is Y X Z, not X Y Z
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);

    let mut movement = Vec3::ZERO;

    if kb_input.pressed(KeyCode::KeyW) {
        movement += transform.forward().as_vec3();
    }
    if kb_input.pressed(KeyCode::KeyA) {
        movement -= transform.right().as_vec3();
    }
    if kb_input.pressed(KeyCode::KeyS) {
        movement -= transform.forward().as_vec3();
    }
    if kb_input.pressed(KeyCode::KeyD) {
        movement += transform.right().as_vec3();
    }
    // up and down use world up instead - more intuitive
    if kb_input.pressed(KeyCode::Space) {
        movement += vec3(0., 1., 0.);
    }
    if kb_input.pressed(KeyCode::ShiftLeft) {
        movement -= vec3(0., 1., 0.);
    }

    let old = transform.translation;
    movement = movement.normalize_or_zero();
    transform.translation += movement * camera_settings.movement_speed * timer.delta_secs();
    if movement != Vec3::ZERO {
        player_moved.write(PlayerMovedEvent {
            old,
            new: transform.translation
        });
    }
}





fn place_and_break(
    mut commands: Commands,
    target: Single<&LookAtData>,
    mut world: Single<&mut BlockWorld>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    block_registry: Res<RegistryHandle<Block>>
) -> Result<(), BevyError> {
    let (Some(pos), Some(face)) = (target.look_pos, target.face) else {
        return Ok(());
    };
    if mouse_input.just_pressed(MouseButton::Left) {
        world.set_block(&mut commands, &pos, BlockState::new("air", &block_registry)?)?;
    }
    else if mouse_input.just_released(MouseButton::Right) {

        let new_pos = pos.offset(face);

        if world.get_block(&new_pos)?.is_air() {
            world.set_block(&mut commands, &new_pos, BlockState::new("stone", &block_registry)?)?;
        }
    }


    Ok(())
}


fn look_at_block(
    mut player: Single<(&mut Transform, &mut LookAtData), With<MainCamera>>,
    world: Single<&BlockWorld>,
    // kb_input: Res<ButtonInput<KeyCode>>,
    // mut gizmos: Gizmos,
) {

    // if !kb_input.just_pressed(KeyCode::KeyF) {
    //     return;
    // }

    let (mut transform, mut look_at_data) = player.into_inner();

    let distance = 5.0;
    let view_dir = transform.forward().as_vec3();
    let pos = transform.translation;

    // gizmos.line(pos, pos + (view_dir * distance), css::GREEN);


    let result = ray::block_raycast(pos, view_dir, distance, |_context, _intersection_point, _face, b_pos| {
        // println!("Testing block {}", b_pos);

        let Ok(block) = world.get_block(&b_pos) else {
            return Ok(false);
        };
        // println!("State: {:?}", block);
        let b = block.is_air();
        let color = if b {
            css::LIGHT_BLUE
        } else {
            css::LIGHT_GREEN
        };

        // let voxel_center = b_pos.center();
        // gizmos.cuboid(Transform::from_translation(voxel_center).with_scale(Vec3::splat(1.0)), color);

        Ok(!b)
    });
    // println!("Result: {:?}", result);
    if let Ok(RayResult::Hit(pos, face, b_pos)) = result {
        // *sphere_vis = Visibility::Visible;
        // look_at_data.translation = pos;
        look_at_data.look_pos = Some(b_pos);
        look_at_data.surface = Some(pos);
        look_at_data.face = Some(face);

        let block = world.get_block(&b_pos).unwrap();

        look_at_data.look_block = Some(block);
    }
    else {
        // *sphere_vis = Visibility::Hidden;
        *look_at_data = LookAtData::default();
    }
}
fn grab_cursor(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // also hide the cursor
    window.cursor_options.visible = false;
}

#[derive(Component, Default)]
pub struct LookAtData {
    pub look_pos: Option<IVec3>,
    pub look_block: Option<BlockState>,
    pub surface: Option<Vec3>,
    pub face: Option<Direction>,
}

#[derive(Component, Default)]
pub struct MainWorld;


fn create_world(
    mut commands: Commands,
) {


    commands.spawn((
        MainWorld::default(),
        BlockWorld::new(),
        MachineWorld::new(),
        ))
        .observe(on_world_join);
}


fn join_world(
    mut commands: Commands,
    q_world: Query<Entity, With<BlockWorld>>,
    camera: Single<&Transform, With<MainCamera>>,
    mut has_run: Local<bool>
) {
    if *has_run {
        return;
    }
    for world in q_world.iter() {
        commands.trigger_targets(JoinedWorldEvent {
            pos: camera.translation
        }, world);
    }
    *has_run = true;
}



fn on_world_join(
    trigger: Trigger<JoinedWorldEvent>,
    mut q_world: Query<&mut BlockWorld>,
) {
    let id = trigger.target();
    let Ok(mut world) = q_world.get_mut(id) else {
        return;
    };
    let world = world.as_mut();

    let chunk_pos = chunk::pos_to_chunk_pos(trigger.pos.as_block_pos());

    let rad = 5;

    let mut queue = VecDeque::new();

    // force map and read_guard to be dropped before queuing chunk generation
    {
        let map = world.get_chunk_map();
        let read_guard = map.read_guard();

        info!("Loading spawn chunks...");
        let mut i = 0;
        for x in -rad..rad + 1 {
            for z in -rad..rad + 1 {
                for y in -rad..rad + 1 {
                    let coord = ivec3(x, y, z) + chunk_pos;
                    if ChunkMap::get_chunk(&coord, &read_guard).is_some() {
                        continue;
                    }

                    queue.push_back(coord);

                    i += 1;
                }
            }
        }
    }

    while !queue.is_empty() {
        world.queue_chunk_generation(queue.pop_front().unwrap());
    }
}



fn on_set_block(
    trigger: Trigger<SetBlockEvent>,
    mut commands: Commands,
    world: Single<&BlockWorld>,
) {

    let map = world.get_chunk_map();
    let read_guard = map.read_guard();

    let pos = trigger.pos;
    let chunk_pos = chunk::pos_to_chunk_pos(pos);

    // Remesh neighboring chunks if needed
    let local_pos = chunk::pos_to_chunk_local(pos);

    let x_axis = if local_pos.x == 0 {
        Some(chunk_pos.west())
    } else if local_pos.x == (ChunkData::CHUNK_SIZE as i32 - 1) {
        Some(chunk_pos.east())
    } else {
        None
    };
    let y_axis = if local_pos.y == 0 {
        Some(chunk_pos.down())
    } else if local_pos.y == (ChunkData::CHUNK_SIZE as i32 - 1) {
        Some(chunk_pos.up())
    } else {
        None
    };
    let z_axis = if local_pos.z == 0 {
        Some(chunk_pos.south())
    } else if local_pos.z == (ChunkData::CHUNK_SIZE as i32 - 1) {
        Some(chunk_pos.north())
    } else {
        None
    };

    println!("x: {:?}, y: {:?}, z: {:?}", x_axis, y_axis, z_axis);


    let chunk = ChunkMap::get_chunk(&chunk_pos, &read_guard).unwrap();
    let entity = chunk.get_entity();
    commands.entity(entity).insert(ChunkNeedsMeshing);

    // remesh neighboring chunks if necessary
    if let Some(x_axis) = x_axis {
        let chunk = ChunkMap::get_chunk(&x_axis, &read_guard).unwrap();
        commands.entity(chunk.get_entity()).insert(ChunkNeedsMeshing);
    }
    if let Some(y_axis) = y_axis {
        let chunk = ChunkMap::get_chunk(&y_axis, &read_guard).unwrap();
        commands.entity(chunk.get_entity()).insert(ChunkNeedsMeshing);
    }
    if let Some(z_axis) = z_axis {
        let chunk = ChunkMap::get_chunk(&z_axis, &read_guard).unwrap();
        commands.entity(chunk.get_entity()).insert(ChunkNeedsMeshing);
    }
}


// Below functions are NOT systems and will be removed at some point
// =================================================================



fn make_data(block_reg: &Registry<Block>) -> ChunkData {
    let mut palette = vec![
        PaletteEntry::new(BlockState::new("air", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("stone", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("dirt", block_reg).unwrap()),
        // PaletteEntry::new("diamond_ore"),
        // PaletteEntry::new("iron_ore"),
    ];

    let id_size = ((palette.len()) as f32).log2().ceil() as usize;

    let mut vec = BitVec::with_capacity(id_size * 32768);

    for y in 0..32 {
        for x in 0..32 {
            for z in 0..32 {

                let id = if(y == 31) {
                    2
                } else {
                    1
                };

                palette[id].increment_ref_count();

                let arr = id.into_bitarray::<Msb0>();
                // println!("Bitarray: {}", arr);

                let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];

                vec.append(&mut slice.to_bitvec())
            }
        }
    }

    ChunkData::with_data(vec, palette)

}


pub fn make_data_chaos(block_reg: &Registry<Block>) -> ChunkData {
    let mut palette = vec![
        PaletteEntry::new(BlockState::new("air", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("stone", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("dirt", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("oak_planks", block_reg).unwrap()),
        // PaletteEntry::new("diamond_ore"),
        // PaletteEntry::new("iron_ore"),
    ];

    // calcualtes the closest power of two id size for the palette.
    let id_size = ((palette.len()) as f32).log2().ceil() as usize;


    let mut vec = BitVec::with_capacity(id_size * 32768);
    let mut rng = rand::rng();

    for i in 0..32768 {
        let scaled_idx = i * id_size;
        // 0-4
        let rand_id = rng.sample(Uniform::new(0, palette.len()).unwrap());

        palette[rand_id].increment_ref_count();
        let arr = rand_id.into_bitarray::<Msb0>();
        // println!("Bitarray: {}", arr);

        let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
        // println!("Slice: {}", slice);
        // println!("Generated num: {}", rand_id);

        vec.append(&mut slice.to_bitvec());
    }

    // println!("{:?}", vec);


    ChunkData::with_data(vec, palette)
}


pub fn make_box(block_reg: &Registry<Block>) -> ChunkData {

    let span = info_span!("make_box").entered();

    let mut palette = vec![
        PaletteEntry::new(BlockState::new("air", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("stone", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("dirt", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("oak_planks", block_reg).unwrap()),
        // PaletteEntry::new("diamond_ore"),
        // PaletteEntry::new("iron_ore"),
    ];


    let id_size = ((palette.len()) as f32).log2().ceil() as usize;

    let mut vec = BitVec::with_capacity(id_size * 32768);
    let mut rng = rand::rng();
    for x in 0..32 {
        for y in 0..32 {
            for z in 0..32 {
                let id = if 12 <= x && x <= 19 && 12 <= y && y <= 19 {
                    if z % 2 == 0 {
                        2
                    }
                    else {
                        0
                    }
                }
                else {
                    0
                };

                palette[id].increment_ref_count();

                let arr = id.into_bitarray::<Msb0>();
                // println!("Bitarray: {}", arr);

                let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
                // println!("Slice: {}", slice);
                // println!("Generated num: {}", rand_id);

                vec.append(&mut slice.to_bitvec());
            }
        }
    }


    ChunkData::with_data(vec, palette)

}



fn height_map_temp(pos: IVec3, block_reg: &Registry<Block>) -> BlockState {
    if pos.y < -4 {
        BlockState::new("stone", block_reg).unwrap()
    }
    else if pos.y <= 0 {
        BlockState::new("dirt", block_reg).unwrap()
    }
    else {
        BlockState::new("air", block_reg).unwrap()
    }
}





fn temp_gen_function(chunk_pos: IVec3, block_reg: &Registry<Block>) -> ChunkData {
    let mut palette = vec![
        PaletteEntry::new(BlockState::new("air", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("stone", block_reg).unwrap()),
        PaletteEntry::new(BlockState::new("dirt", block_reg).unwrap()),
    ];

    let id_size = ((palette.len()) as f32).log2().ceil() as usize;

    let mut vec = BitVec::with_capacity(id_size * ChunkData::BLOCKS_PER_CHUNK);

    // Data is stored Z -> X -> Y, so we iterate over all z first then all x then all y.
    for y in 0..ChunkData::CHUNK_SIZE {
        for x in 0..ChunkData::CHUNK_SIZE {
            for z in 0..ChunkData::CHUNK_SIZE {
                let block_pos = chunk::chunk_pos_to_world_pos(chunk_pos) + ivec3(x as i32, y as i32, z as i32);

                // all of this is temporary lol
                let state = height_map_temp(block_pos, block_reg);
                let id = match state.get_id() {
                    "air" => 0,
                    "stone" => 1,
                    "dirt" => 2,
                    _ => unreachable!(),
                };



                palette[id].increment_ref_count();

                // if block_pos.y > 0 && id == 2 {
                //     println!("Why is this dirt? {}, local: {}", block_pos, ivec3(x as i32, y as i32, z as i32));
                // }



                let arr = id.into_bitarray::<Msb0>();
                let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];

                vec.append(&mut slice.to_bitvec());
            }
        }
    }


    ChunkData::with_data(vec, palette)

}