use std::collections::{HashMap, VecDeque};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use bevy::asset::AssetContainer;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::*;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use bevy::tasks::futures_lite::future;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bitvec::order::Msb0;
use bitvec::prelude::BitVec;
use bitvec::view::BitViewSized;
use rand::distr::Uniform;
use rand::Rng;
use cache::ChunkCache;
use crate::asset::block::{Block, BlockModel};
use crate::render::material::BlockMaterial;
use crate::asset::procedural::BlockTextures;
use crate::core::state::MainGameState;
use crate::registry::block::BlockRegistry;
use crate::render::MeshDataCache;
use crate::world::camera::{CameraSettings, MainCamera};
use crate::world::chunk::{ChunkComponent, ChunkData, ChunkNeedsMeshing, PaletteEntry};

pub mod chunk;
mod camera;
mod cache;

#[derive(Default)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraSettings>()
            // temp
            .init_resource::<StartedGenerating>()

            .add_systems(Update, (handle_input, ))
            .add_systems(FixedUpdate, queue_mesh_creation)
            .add_systems(Update, (temp_create_chunk, receive_generated_chunks, create_chunk_entities))
            .add_systems(Update, (receive_generated_meshes, upload_meshes, create_materials))
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

    mut materials: ResMut<Assets<StandardMaterial>>,
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

    // just so i can see a reference to 0 0 0
    commands.spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
        Mesh3d(meshes.add(Sphere {radius: 3.0}.mesh())),
    ));
}

#[derive(Component, Debug, Default)]
pub struct GameWorld;


fn create_world(
    mut commands: Commands,
) {
    commands.spawn((
        GameWorld,
        ChunkCache::default(),
        ChunkQueue::default(),
        ));
}

#[derive(Component, Debug, Default)]
struct ChunkQueue {

    currently_generating: HashMap<IVec3, Task<ChunkData>>,
    finished_generating: VecDeque<(IVec3, ChunkData)>,
    currently_meshing: HashMap<IVec3, Task<Option<Mesh>>>,
    finished_meshing: VecDeque<(IVec3, Option<Mesh>)>,
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
fn create_chunk_entities(
    mut commands: Commands,
    mut world: Single<(&mut ChunkQueue, &mut ChunkCache)>,
    mut started_generating: ResMut<StartedGenerating>,
) {
    let (mut chunk_queue, mut chunk_cache) = world.into_inner();

    let mut print = false;
    while !chunk_queue.finished_generating.is_empty() {
        let (coord, data) = chunk_queue.finished_generating.pop_front().unwrap();

        let e = commands.spawn((
            Visibility::Visible,
            chunk::pos_to_transform(coord),
            ChunkComponent::new(coord, data),
            ChunkNeedsMeshing
        )).id();
        chunk_cache.add_to_cache(coord, e);
        print = true;
    }
    if print && chunk_queue.currently_generating.is_empty() && started_generating.0 {
        println!("Finished generating chunks.");
        started_generating.0 = false;
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

// how many MiB per frame can we upload to the GPU? Default 1.
const MIB_PER_FRAME: i32 = 1024 * 1024 * 1;


#[derive(Component)]
pub struct MeshRef(pub Arc<Handle<Mesh>>);

#[derive(Component)]
pub struct NeedsMaterial;

fn upload_meshes(
    mut commands: Commands,
    world: Single<(&mut ChunkQueue, &mut ChunkCache)>,
    mut q_chunks: Query<&mut ChunkComponent>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (mut chunk_queue, chunk_cache) = world.into_inner();

    // if !chunk_queue.finished_meshing.is_empty() {
    //     println!("Currently meshing queue size: {}", chunk_queue.currently_meshing.len());
    //     println!("Chunk queue size: {}", chunk_queue.finished_meshing.len());
    // }


    // let mut new_entities = Vec::new();
    let mut hard_process_limit = MIB_PER_FRAME;
    while !chunk_queue.finished_meshing.is_empty() && hard_process_limit > 0 {

        let (coord, mesh) = chunk_queue.finished_meshing.pop_front().unwrap();

        // air - we don't need to make a mesh and can just move on
        if mesh.is_none() {
            continue;
        }

        let mesh = mesh.unwrap();


        // println!("Indices: {mesh_size}");

        // println!("Buffer size: {}, vertex size: {}, num vertices: {}", mesh.get_vertex_buffer_size(), mesh.get_vertex_size(), mesh.count_vertices());

        // scales the amount of "work" done by how big this mesh is
        // if the mesh is very big, less meshes will be uploaded this frame.
        let to_sub = mesh.get_vertex_buffer_size();

        // println!("Coord: {}, count: {}", coord, counter.count);

        let entity = chunk_cache.get_chunk(coord).expect("Can't remesh chunk that isn't in memory!");
        // let mut component = q_chunks.get_mut(entity).expect("Invalid entity id");

        // will replace the mesh if it already has one.
        let mesh_ref = MeshRef(Arc::new(meshes.add(mesh)));

        let child = commands.spawn((
            Visibility::Inherited,
            // chunk::pos_to_transform(coord),
            mesh_ref,
            NeedsMaterial,
        )).id();


        commands.entity(entity).add_child(child);
        // counter.count += 1;
        hard_process_limit -= to_sub as i32;
    }
}

fn create_materials(
    mut commands: Commands,
    mut chunks: Query<(Entity, &MeshRef), With<NeedsMaterial>>,
    block_textures: Res<BlockTextures>,
) {
    for (entity, mesh_ref) in chunks.iter_mut() {
        commands
            .entity(entity)
            .insert(Mesh3d((*mesh_ref.0).clone()))
            .insert(MeshMaterial3d(block_textures.material.clone()))
            .remove::<NeedsMaterial>()
        ;
    }
}




fn queue_mesh_creation(
    mut commands: Commands,
    camera: Single<&Transform, With<MainCamera>>,
    mut world: Single<(&mut ChunkQueue, &mut ChunkCache)>,
    needs_mesh: Query<(Entity, &ChunkComponent), With<ChunkNeedsMeshing>>,
    q_chunks: Query<&ChunkComponent>,
    mut mesh_cache: Res<MeshDataCache>,

) {

    //TODO: do by player render distance

    let (mut chunk_queue, chunk_cache) = world.into_inner();

    for (entity, chunk) in needs_mesh.iter() {
        if chunk_queue.currently_meshing.contains_key(&chunk.pos) {
            continue;
        }


        // create a new mesh

        //TODO: check if neighbors are in cache, if not - can't mesh.

        let north = chunk_cache.get_chunk(chunk.pos + ivec3(0, 0, 1));
        let south = chunk_cache.get_chunk(chunk.pos + ivec3(0, 0, -1));
        let east = chunk_cache.get_chunk(chunk.pos + ivec3(1, 0, 0));
        let west = chunk_cache.get_chunk(chunk.pos + ivec3(-1, 0, 0));
        let up = chunk_cache.get_chunk(chunk.pos + ivec3(0, 1, 0));
        let down = chunk_cache.get_chunk(chunk.pos + ivec3(0, -1, 0));
        if let (Some(north), Some(south), Some(east), Some(west), Some(up), Some(down)) = (north, south, east, west, up, down) {


            commands.entity(entity).remove::<ChunkNeedsMeshing>();

            let cache = mesh_cache.clone();
            let chunk_arc = chunk.borrow_data();
            let north_arc = q_chunks.get(north).unwrap().borrow_data();
            let south_arc = q_chunks.get(south).unwrap().borrow_data();
            let east_arc = q_chunks.get(east).unwrap().borrow_data();
            let west_arc = q_chunks.get(west).unwrap().borrow_data();
            let up_arc = q_chunks.get(up).unwrap().borrow_data();
            let down_arc = q_chunks.get(down).unwrap().borrow_data();

            let task = AsyncComputeTaskPool::get().spawn(async move {
                // read the data
                let data = chunk_arc.read().unwrap();
                let north_data = north_arc.read().unwrap();
                let south_data = south_arc.read().unwrap();
                let east_data = east_arc.read().unwrap();
                let west_data = west_arc.read().unwrap();
                let up_data = up_arc.read().unwrap();
                let down_data = down_arc.read().unwrap();
                let neighbors: chunk::NeighborData = (
                    &north_data,
                    &south_data,
                    &east_data,
                    &west_data,
                    &up_data,
                    &down_data,
                );


                if data.is_empty() {
                    None
                }
                else {
                    // create the mesh
                    Some(chunk::create_chunk_mesh(&data, &cache, Some(neighbors)))
                }

            });
            chunk_queue.currently_meshing.insert(chunk.pos, task);
        }

    }



        // let cache = mesh_cache.clone();
        // let chunk_arc = chunk.borrow_data();
        // let task = AsyncComputeTaskPool::get().spawn(async move {
        //     let data = chunk_arc.read().unwrap();
        //     chunk::create_chunk_mesh(&data, &cache, None)
        // });
        // if chunk_queue.currently_meshing.contains_key(&chunk.pos) {
        //     panic!("Submitting a mesh task that has already been submitted for chunk {}", chunk.pos);
        // }
        // chunk_queue.currently_meshing.insert(chunk.pos, task);
}

#[derive(Default, Copy, Clone, Debug, Resource)]
struct StartedGenerating(bool);

fn temp_create_chunk(
    mut chunk_queue: Single<&mut ChunkQueue>,
    kb_input: Res<ButtonInput<KeyCode>>,
    mut started_generating: ResMut<StartedGenerating>,
) {
    // if kb_input.just_pressed(KeyCode::KeyX) {
    //
    //     let task = AsyncComputeTaskPool::get().spawn(async move {
    //         // let now = Instant::now();
    //         let ret = make_data();
    //         // let elapsed = now.elapsed().as_secs_f64();
    //         // info!("Created chunk data. Took {} seconds,", elapsed);
    //         ret
    //     });
    //     chunk_queue.currently_generating.insert(ivec3(chunk_spawner.x, 0, chunk_spawner.z), task);
    //
    //     chunk_spawner.x += 1;
    //     // chunk_spawner.z += 1;
    // }

    let rad = 5;

    if kb_input.just_pressed(KeyCode::KeyX) {
        info!("Loading chunks...");
        let mut i = 0;
        for x in -rad..rad + 1 {
            for z in -rad..rad + 1 {
                for y in -rad..rad + 1 {
                    let task = AsyncComputeTaskPool::get().spawn(async move {
                        make_box()
                    });
                    chunk_queue.currently_generating.insert(ivec3(x, y, z), task);
                    i += 1;
                }
            }
        }
        info!("Created {i} chunk tasks.");
        started_generating.0 = true;
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


pub fn make_data_chaos() -> ChunkData {
    let mut palette = vec![
        PaletteEntry::new("stone"),
        PaletteEntry::new("dirt"),
        PaletteEntry::new("oak_planks"),
        // PaletteEntry::new("diamond_ore"),
        // PaletteEntry::new("iron_ore"),
    ];

    // calcualtes the closest power of two id size for the palette.
    let id_size = ((palette.len() + 1) as f32).log2().ceil() as usize;


    let mut vec = BitVec::with_capacity(id_size * 32768);
    let mut rng = rand::rng();

    for i in 0..32768 {
        let scaled_idx = i * id_size;
        // 0-4
        let rand_id = rng.sample(Uniform::new(0, palette.len() + 1).unwrap());

        if rand_id != 0 {
            palette[rand_id - 1].increment_ref_count();
        }
        let arr = rand_id.into_bitarray::<Msb0>();
        // println!("Bitarray: {}", arr);

        let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
        // println!("Slice: {}", slice);
        // println!("Generated num: {}", rand_id);

        vec.append(&mut slice.to_bitvec());
    }

    // println!("{:?}", vec);


    ChunkData::new(vec, palette)
}


pub fn make_box() -> ChunkData {
    let mut palette = vec![
        PaletteEntry::new("stone"),
        PaletteEntry::new("dirt"),
        PaletteEntry::new("oak_planks"),
        // PaletteEntry::new("diamond_ore"),
        // PaletteEntry::new("iron_ore"),
    ];


    let id_size = ((palette.len() + 1) as f32).log2().ceil() as usize;

    let mut vec = BitVec::with_capacity(id_size * 32768);
    let mut rng = rand::rng();
    for x in 0..32 {
        for y in 0..32 {
            for z in 0..32 {
                let id = if 12 <= x && x <= 19 && 12 <= y && y <= 19 {
                    rng.sample(Uniform::new(1, palette.len() + 1).unwrap())
                }
                else {
                    0
                };

                if id != 0 {
                    palette[id - 1].increment_ref_count();
                }
                let arr = id.into_bitarray::<Msb0>();
                // println!("Bitarray: {}", arr);

                let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
                // println!("Slice: {}", slice);
                // println!("Generated num: {}", rand_id);

                vec.append(&mut slice.to_bitvec());
            }
        }
    }


    ChunkData::new(vec, palette)

}