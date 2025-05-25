use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::*;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use bevy::tasks::futures_lite::future;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bitvec::order::Msb0;
use bitvec::prelude::BitVec;
use bitvec::view::BitViewSized;
use crate::{TestChunk};
use crate::asset::block::{Block, BlockModel};
use crate::render::material::BlockMaterial;
use crate::asset::procedural::BlockTextures;
use crate::core::state::MainGameState;
use crate::registry::block::BlockRegistry;
use crate::render::MeshDataCache;
use crate::world::camera::{CameraSettings, MainCamera};
use crate::world::chunk::{ChunkData, PaletteEntry};

pub mod chunk;
mod camera;

#[derive(Default)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraSettings>()
            // temp
            .init_resource::<ChunkSpawner>()

            .add_systems(Update, (handle_input, ))
            .add_systems(Update, (temp_create_chunk, receive_generated_chunks, start_generating_meshes, receive_generated_meshes, upload_meshes).chain())
            .add_systems(OnEnter(MainGameState::InGame), (setup, grab_cursor, create_world))
        ;
    }
}

fn handle_input(
    mut transform: Single<&mut Transform, With<MainCamera>>,
    // mut proj: Single<&mut Projection, With<MainCamera>>,
    camera_settings: Res<CameraSettings>,
    timer: Res<Time>,
    kb_input: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
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

    movement = movement.normalize_or_zero();
    transform.translation += movement * camera_settings.movement_speed * timer.delta_secs();
}


fn grab_cursor(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // also hide the cursor
    window.cursor_options.visible = false;
}

// runs once when InGame reached
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    block_data_cache: Res<MeshDataCache>,
    block_textures: Res<BlockTextures>,
    camera_settings: Res<CameraSettings>,
    level: Res<TestChunk>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("Loading world...");

    // let texture1: Handle<Image> = asset_server.load("texture/stone.png");
    // let texture2: Handle<Image> = asset_server.load("dirt.png");
    // let texture3: Handle<Image> = asset_server.load("oak_planks.png");

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

    let array_texture = block_textures.array_texture.clone();

    let mesh = meshes.add(chunk::create_chunk_mesh(&level.inner, &block_data_cache));

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(block_textures.material.clone()),
        Transform::from_xyz(0., 0., 0.)
    ));
}

#[derive(Component, Debug, Default)]
pub struct World;


#[derive(Component, Debug, Default)]
pub struct ChunkCache {
    map: HashMap<(i32, i32, i32), Box<ChunkData>>
}
impl ChunkCache {
    pub fn get(&self, x: i32, y: i32, z: i32) -> &Box<ChunkData> {
        if self.map.contains_key(&(x, y, z)) {
            &self.map[&(x, y, z)]
        }
        else {
            todo!("Create chunks that haven't been loaded yet")
        }
    }
}


fn create_world(
    mut commands: Commands,
) {
    commands.spawn((
            World,
            ChunkCache::default(),
            ChunkQueue::default(),
        ));
}

#[derive(Component, Debug, Default)]
struct ChunkQueue {

    currently_generating: HashMap<IVec3, Task<ChunkData>>,
    finished_generating: VecDeque<(IVec3, ChunkData)>,
    currently_meshing: HashMap<IVec3, Task<Mesh>>,
    finished_meshing: VecDeque<(IVec3, Mesh)>,
}

// receives chunks that have finished generating.
fn receive_generated_chunks(
    mut chunk_queue: Single<&mut ChunkQueue>,
) {
    let mut finished = VecDeque::new();
    // this needs to be in a separate scope so the first mutable reference can be dropped.
    {
        for (coord, task) in chunk_queue.currently_generating.iter_mut() {
            let task_status = block_on(future::poll_once(task));
            if task_status.is_none() {
                continue;
            }
            let data = task_status.unwrap();
            finished.push_back((coord.clone(), data));
        }
    }

    while !finished.is_empty() {
        let (coord, data) = finished.pop_front().unwrap();
        chunk_queue.currently_generating.remove(&coord);
        chunk_queue.finished_generating.push_back((coord.clone(), data));
    }
}

// all generated chunks start to mesh
fn start_generating_meshes(
    mut chunk_queue: Single<&mut ChunkQueue>,
    mesh_cache: Res<MeshDataCache>,
) {
    while !chunk_queue.finished_generating.is_empty() {
        let (coord, data) = chunk_queue.finished_generating.pop_front().unwrap();
        let arc_data = Arc::new(data);

        let cache = Arc::new(mesh_cache.clone());
        let task = AsyncComputeTaskPool::get().spawn(async move {
            chunk::create_chunk_mesh(arc_data.as_ref(), &cache)
        });
        chunk_queue.currently_meshing.insert(coord, task);
    }
}

fn receive_generated_meshes(
    mut chunk_queue: Single<&mut ChunkQueue>,
) {
    let mut finished = VecDeque::new();
    {
        for (coord, task) in chunk_queue.currently_meshing.iter_mut() {

            let task_status = block_on(future::poll_once(task));
            if task_status.is_none() {
                continue;
            }
            let mesh = task_status.unwrap();
            finished.push_back((coord.clone(), mesh));
        }
    }
    while !finished.is_empty() {
        let (coord, mesh) = finished.pop_front().unwrap();
        chunk_queue.currently_meshing.remove(&coord);
        chunk_queue.finished_meshing.push_back((coord.clone(), mesh));

    }
}

fn upload_meshes(
    mut commands: Commands,
    mut chunk_queue: Single<&mut ChunkQueue>,
    mut meshes: ResMut<Assets<Mesh>>,
    block_textures: Res<BlockTextures>

) {
    while !chunk_queue.finished_meshing.is_empty() {
        let (coord, mesh) = chunk_queue.finished_meshing.pop_front().unwrap();

        let transform = Transform::from_xyz((coord.x * ChunkData::CHUNK_SIZE as i32) as f32, (coord.y * ChunkData::CHUNK_SIZE as i32) as f32, (coord.z * ChunkData::CHUNK_SIZE as i32) as f32);

        commands.spawn((
            transform,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(block_textures.material.clone()),
            // chunk_data,
        ));
    }
}


#[derive(Resource, Debug, Clone)]
struct ChunkSpawner {
    data: ChunkData,
    x: i32,
    z: i32
}
impl Default for ChunkSpawner {
    fn default() -> Self {
        Self {
            data: make_data(),
            x: 1,
            z: 1
        }
    }
}

fn temp_create_chunk(
    mut chunk_queue: Single<&mut ChunkQueue>,
    mut chunk_spawner: ResMut<ChunkSpawner>,
    kb_input: Res<ButtonInput<KeyCode>>,
) {
    // if kb_input.just_pressed(KeyCode::KeyX) {
    //
    //     let task = AsyncComputeTaskPool::get().spawn(async move {
    //         make_data()
    //     });
    //     chunk_queue.currently_generating.insert(ivec3(chunk_spawner.x, 0, chunk_spawner.z), task);
    //
    //     chunk_spawner.x += 1;
    //     // chunk_spawner.z += 1;
    // }

    if kb_input.just_pressed(KeyCode::KeyX) {
        for x in -20..21 {
            for z in -20..21 {
                if x == 0 && z == 0 {
                    continue;
                }

                let task = AsyncComputeTaskPool::get().spawn(async move {
                    make_data()
                });
                chunk_queue.currently_generating.insert(ivec3(x, 0, z), task);
            }
        }
    }
}



fn make_data() -> ChunkData {
    let mut palette = vec![
        PaletteEntry::new("stone"),
        PaletteEntry::new("dirt"),
        // PaletteEntry::new("diamond_ore"),
        // PaletteEntry::new("iron_ore"),
    ];

    let id_size = ((palette.len() + 1) as f32).log2().ceil() as usize;

    let mut vec = BitVec::with_capacity(id_size * 32768);

    for y in 0..32 {
        for x in 0..32 {
            for z in 0..32 {

                let id = if(y == 31) {
                    2
                } else {
                    1
                };

                palette[id - 1].increment_ref_count();

                let arr = id.into_bitarray::<Msb0>();
                // println!("Bitarray: {}", arr);

                let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];

                vec.append(&mut slice.to_bitvec())
            }
        }
    }

    ChunkData::new(vec, palette)

}