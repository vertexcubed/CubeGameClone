use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use bevy::asset::AssetContainer;
use bevy::color::palettes::css;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::math::bounding::{Aabb3d, IntersectsVolume};
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::*;
use bevy::render::primitives::Aabb;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use bevy::tasks::futures_lite::future;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bitvec::order::Msb0;
use bitvec::prelude::BitVec;
use bitvec::view::BitViewSized;
use rand::distr::Uniform;
use rand::Rng;
use source::ChunkMap;
use crate::asset::block::BlockModelAsset;
use crate::render::material::BlockMaterial;
use crate::asset::procedural::BlockTextures;
use crate::core::errors::WorldError;
use crate::core::event::{PlayerMovedEvent, SetBlockEvent};
use crate::core::state::MainGameState;
use crate::math::ray;
use crate::math::block::{BlockPos, Vec3Ext};
use crate::math::ray::RayResult;
use crate::registry::block::Block;
use crate::registry::{Registry, RegistryHandle};
use crate::render;
use crate::render::block::MeshDataCache;
use crate::world::block::BlockState;
use crate::world::camera::{CameraSettings, MainCamera};
use crate::world::chunk::{Chunk, ChunkData, ChunkNeedsMeshing, PaletteEntry};
use crate::world::source::WorldSource;

pub mod chunk;
pub mod camera;
pub mod source;
pub mod block;

#[derive(Default)]
pub struct GameWorldPlugin;

impl Plugin for GameWorldPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraSettings>()
            // temp

            .add_systems(Update, (handle_input, raycast))
            .add_systems(Update, (temp_create_chunk, temp_set_block).chain())
            // .add_systems(Update, track_chunks_around_player)
            .add_systems(PostUpdate, on_set_block)
            .add_systems(OnEnter(MainGameState::InGame), (setup, grab_cursor, create_world))
        ;
        source::add_systems(app);
    }
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


fn raycast(
    mut transform: Single<&mut Transform, (With<MainCamera>, Without<CursorTemp>)>,
    world: Single<&WorldSource>,
    kb_input: Res<ButtonInput<KeyCode>>,
    sphere: Single<(&mut Transform, &mut Visibility, &mut CursorTemp)>,
    mut gizmos: Gizmos,
) {

    // if !kb_input.just_pressed(KeyCode::KeyF) {
    //     return;
    // }

    let (mut sphere, mut sphere_vis, mut cursor) = sphere.into_inner();

    let distance = 5.0;
    let view_dir = transform.forward().as_vec3();
    let pos = transform.translation;

    // gizmos.line(pos, pos + (view_dir * distance), css::GREEN);


    let result = ray::block_raycast(pos, view_dir, distance, |_context, _intersection_point, b_pos| {
        // println!("Testing block {}", b_pos);

        let Ok(block) = world.get_block(&b_pos) else {
            return false;
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

        !b
    });
    // println!("Result: {:?}", result);
    if let Ok(RayResult::Hit(pos, b_pos)) = result {
        *sphere_vis = Visibility::Visible;
        sphere.translation = pos;
        cursor.look_pos = Some(b_pos);
        cursor.surface = Some(pos);

        let block = world.get_block(&b_pos).unwrap();

        cursor.look_block = Some(block);
    }
    else {
        *sphere_vis = Visibility::Hidden;
        *cursor = CursorTemp::default();
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
pub struct CursorTemp {
    pub look_pos: Option<IVec3>,
    pub look_block: Option<BlockState>,
    pub surface: Option<Vec3>
}

// runs once when InGame reached
fn setup(
    mut commands: Commands,
    camera_settings: Res<CameraSettings>,


    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
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
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(25.0, 50.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y)
    ));

    commands.spawn((
        CursorTemp::default(),
        Transform::from_xyz(0.0, 0.0, 0.0),
        MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
        Mesh3d(meshes.add(Sphere {radius: 0.125}.mesh())),
    ));

}

#[derive(Component)]
pub struct GameWorld {
    // pub generator_func: Arc<dyn Fn(IVec3) -> ChunkData + Send + Sync>
}
impl Default for GameWorld {
    fn default() -> Self {
        Self {
            // generator_func: Arc::new(|i| {
            //     make_box()
            // })
        }
    }
}


fn create_world(
    mut commands: Commands,
) {
    commands.spawn((
        GameWorld::default(),
        WorldSource::new()
        ));
}


fn on_set_block(
    mut commands: Commands,
    world: Single<&WorldSource>,
    mut event: EventReader<SetBlockEvent>,
) {
    if event.is_empty() {
        return;
    }
    
    let map = world.get_chunk_map();
    let read_guard = map.read_guard();
    
    
    for e in event.read() {
        
        let pos = e.pos;
        let chunk_pos = chunk::pos_to_chunk_pos(pos);
        let chunk = ChunkMap::get_chunk(&chunk_pos, &read_guard).unwrap();
        let entity = chunk.get_entity();
        commands.entity(entity).insert(ChunkNeedsMeshing);
    }
}



fn temp_create_chunk(
    camera: Single<&Transform, With<MainCamera>>,
    mut world: Single<&mut WorldSource>,
    kb_input: Res<ButtonInput<KeyCode>>,
    block_reg: Res<RegistryHandle<Block>>,
) {
    let camera_chunk = chunk::transform_to_chunk_pos(&camera);

    let rad = 5;

    let mut queue = VecDeque::new();

    if kb_input.just_pressed(KeyCode::KeyX) {
        let mut map = world.get_chunk_map_mut();
        let read_guard = map.read_guard();
        
        info!("Loading chunks...");
        let mut i = 0;
        for x in -rad..rad + 1 {
            for z in -rad..rad + 1 {
                for y in -rad..rad + 1 {
                    let coord = ivec3(x, y, z) + camera_chunk;
                    if ChunkMap::get_chunk(&coord, &read_guard).is_some() {
                        continue;
                    }
                    
                    queue.push_back(coord);
                    
                    i += 1;
                }
            }
        }
        info!("Created {i} chunk tasks.");
    }
    
    while !queue.is_empty() {
        world.queue_chunk_generation(queue.pop_front().unwrap());
    }
}


fn temp_set_block(
    mut world: Single<&mut WorldSource>,
    kb_input: Res<ButtonInput<KeyCode>>,
    camera: Single<&Transform, With<MainCamera>>,
    block_reg: Res<RegistryHandle<Block>>,
) {

    if kb_input.just_pressed(KeyCode::KeyV) {
        let camera_chunk = chunk::transform_to_chunk_pos(&camera);
        let pos = camera.translation.floor().as_ivec3();
        let pos_in_chunk = chunk::pos_to_chunk_local(pos);
        info!("Pos: {}, camera chunk: {}, pos in chunk: {}", pos, camera_chunk, pos_in_chunk);
    }

    if kb_input.just_pressed(KeyCode::KeyC) {
        // chunk coord of camera

        let camera_pos = camera.translation.as_block_pos();
        let new = BlockState::new("oak_slab", block_reg.as_ref()).unwrap();

        match world.set_block(&camera_pos, new.clone()) {
            Ok(old) => {
                info!("Block set. Old: {:?}, new: {:?}", old, new);
            }
            Err(e) => {
                error!("Error setting block: {e}");
            }
        }
    }
}


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