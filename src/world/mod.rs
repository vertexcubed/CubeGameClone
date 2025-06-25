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
use cache::ChunkCache;
use crate::asset::block::{BlockModelAsset};
use crate::render::material::BlockMaterial;
use crate::asset::procedural::BlockTextures;
use crate::core::event::PlayerMovedEvent;
use crate::core::state::MainGameState;
use crate::math::{ray, Vec3Ext};
use crate::math::ray::RayResult;
use crate::registry::block::Block;
use crate::registry::{RegistryHandle, Registry};
use crate::render::block::MeshDataCache;
use crate::world::block::BlockState;
use crate::world::camera::{CameraSettings, MainCamera};
use crate::world::chunk::{ChunkComponent, ChunkData, ChunkNeedsMeshing, PaletteEntry};

pub mod chunk;
pub mod camera;
pub mod cache;
pub mod block;

#[derive(Default)]
pub struct GameWorldPlugin;

impl Plugin for GameWorldPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraSettings>()
            // temp
            .init_resource::<StartedGenerating>()

            .add_systems(Update, (handle_input, raycast))
            .add_systems(FixedUpdate, (queue_mesh_creation, ))
            .add_systems(Update, (temp_create_chunk, temp_set_block, receive_generated_chunks, create_chunk_entities).chain())
            .add_systems(Update, (receive_generated_meshes, upload_meshes))
            .add_systems(Update, track_chunks_around_player)
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
    chunk_cache: Single<&ChunkCache>,
    q_chunks: Query<&ChunkComponent>,
    kb_input: Res<ButtonInput<KeyCode>>,

    sphere: Single<(&mut Transform, &mut Visibility, &mut CursorTemp)>,

    mut gizmos: Gizmos,


    mut commands: Commands,
) {

    // if !kb_input.just_pressed(KeyCode::KeyF) {
    //     return;
    // }

    let (mut sphere, mut sphere_vis, mut cursor) = sphere.into_inner();

    let distance = 5.0;
    let view_dir = transform.forward().as_vec3();
    let pos = transform.translation;

    // gizmos.line(pos, pos + (view_dir * distance), css::GREEN);


    let result = ray::block_raycast(pos, view_dir, distance, |b_pos| {
        // println!("Testing block {}", b_pos);

        let voxel_center = b_pos.as_vec3() + 0.5;
        // gizmos.cuboid(Transform::from_translation(voxel_center).with_scale(Vec3::splat(1.0)), css::PINK);

        let chunk = chunk::pos_to_chunk_pos(b_pos);
        let local = chunk::pos_to_chunk_local(b_pos);
        let entity = chunk_cache.get_chunk(chunk);
        if entity.is_none() {
            return false;
        }
        let component = q_chunks.get(entity.unwrap()).unwrap();
        let block = component.get_block(local);
        // println!("State: {:?}", block);
        !block.is_air()
    });
    // println!("Result: {:?}", result);
    if let Ok(RayResult::Hit(pos, b_pos)) = result {
        *sphere_vis = Visibility::Visible;
        sphere.translation = pos;
        cursor.look_pos = Some(b_pos);
        cursor.surface = Some(pos);

        let chunk = chunk::pos_to_chunk_pos(b_pos);
        let local = chunk::pos_to_chunk_local(b_pos);
        let entity = chunk_cache.get_chunk(chunk);
        let component = q_chunks.get(entity.unwrap()).unwrap();
        let block = component.get_block(local);

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
        ChunkCache::default(),
        ChunkQueue::default(),
        ));
}


//TODO: switch "finished" queues to priority queues to allow O(logn) lookup.
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
            chunk::chunk_pos_to_transform(coord),
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
pub struct ChunkMeshMarker;


fn upload_meshes(
    mut commands: Commands,
    world: Single<(&mut ChunkQueue, &mut ChunkCache)>,
    q_children: Query<&Children, With<ChunkComponent>>,
    q_chunk_meshes: Query<&ChunkMeshMarker>,
    mut meshes: ResMut<Assets<Mesh>>,
    block_textures: Res<BlockTextures>,
) {
    let span = info_span!("upload_meshes").entered();

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

        let chunk_entity = chunk_cache.get_chunk(coord).expect("Can't remesh chunk that isn't in memory!");
        // let mut component = q_chunks.get_mut(entity).expect("Invalid entity id");



        // create the mesh handle
        let mesh_handle = meshes.add(mesh);

        let mut needs_new_mesh = true;
        // chunk may or may not already have a mesh.
        if let Ok(children) = q_children.get(chunk_entity) {
            //iter over all the children.
            for child in children.iter() {
                // does this child have a mesh?
                if q_chunk_meshes.contains(child) {
                    commands.entity(child).insert(Mesh3d(mesh_handle.clone()));
                    needs_new_mesh = false;
                }
            }
        }
        if needs_new_mesh {
            let child = commands.spawn((
                    Visibility::Inherited,
                    Mesh3d(mesh_handle.clone()),
                    ChunkMeshMarker,
                    MeshMaterial3d(block_textures.material.clone()),
                )).id();

                commands.entity(chunk_entity).add_child(child);
        }
        hard_process_limit -= to_sub as i32;
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
    camera: Single<&Transform, With<MainCamera>>,
    mut world: Single<(&GameWorld, &mut ChunkQueue, &ChunkCache)>,
    kb_input: Res<ButtonInput<KeyCode>>,
    mut started_generating: ResMut<StartedGenerating>,
    block_reg: Res<RegistryHandle<Block>>,
) {
    let (game_world, mut chunk_queue, chunk_cache) = world.into_inner();

    let camera_chunk = chunk::transform_to_chunk_pos(&camera);


    let rad = 5;
    
    
    if kb_input.just_pressed(KeyCode::KeyX) {
        info!("Loading chunks...");
        let mut i = 0;
        for x in -rad..rad + 1 {
            for z in -rad..rad + 1 {
                for y in -rad..rad + 1 {
                    let coord = ivec3(x, y, z) + camera_chunk;
                    if chunk_cache.get_chunk(coord).is_some() {
                        continue;
                    }
                    
                    let reg = block_reg.clone();
                    
                    let task = AsyncComputeTaskPool::get().spawn(async move {
                        make_box(reg.as_ref())
                    });
                    chunk_queue.currently_generating.insert(coord, task);
                    i += 1;
                }
            }
        }
        info!("Created {i} chunk tasks.");
        started_generating.0 = true;
    }
}


fn temp_set_block(
    mut commands: Commands,
    mut q_chunks: Query<&mut ChunkComponent>,
    world: Single<(&GameWorld, &ChunkQueue, &ChunkCache)>,
    kb_input: Res<ButtonInput<KeyCode>>,
    camera: Single<&Transform, With<MainCamera>>,
    block_reg: Res<RegistryHandle<Block>>,
) {
    let (game_world, chunk_queue, chunk_cache) = world.into_inner();

    if kb_input.just_pressed(KeyCode::KeyV) {
        let camera_chunk = chunk::transform_to_chunk_pos(&camera);
        let pos = camera.translation.floor().as_ivec3();
        let pos_in_chunk = chunk::pos_to_chunk_local(pos);
        info!("Pos: {}, camera chunk: {}, pos in chunk: {}", pos, camera_chunk, pos_in_chunk);
    }

    if kb_input.just_pressed(KeyCode::KeyB) {
        let center = IVec3::ZERO;

        let pos1 = ivec3(0, 0, 0);
        let pos2 = ivec3(1, 0, 0);
        if let Some(entity) = chunk_cache.get_chunk(center) {
            let mut component = q_chunks.get_mut(entity).unwrap();

            let planks = BlockState::new("oak_planks", block_reg.as_ref()).unwrap();
            component.set_block(pos1, planks.clone());
            component.set_block(pos2, planks);
            commands.entity(entity).insert(ChunkNeedsMeshing);
        }
    }

    if kb_input.just_pressed(KeyCode::KeyC) {
        // chunk coord of camera
        let camera_chunk = chunk::transform_to_chunk_pos(&camera);
        let pos = camera.translation.as_block_pos();

        let pos_in_chunk = chunk::pos_to_chunk_local(pos);
        if let Some(entity) = chunk_cache.get_chunk(camera_chunk) {
            let mut component = q_chunks.get_mut(entity).unwrap();

            info!("Old: {:?}", component.get_block(pos_in_chunk));

            let stone = BlockState::new("oak_slab", block_reg.as_ref()).unwrap();


            match component.set_block(pos_in_chunk, stone) {
                Ok(old) => {
                    info!("Set block at {}. Old: {:?}", pos, old);
                    commands.entity(entity).insert(ChunkNeedsMeshing);
                }
                Err(e) => {
                    error!("Failed to set block at {}: {:?}", pos, e);
                }
            }

            info!("New: {:?}", component.get_block(pos_in_chunk));

        }
        else {
            info!("Can't set block at {}, chunk not in cache.", pos);
        }
    }
}


fn track_chunks_around_player(
    mut commands: Commands,
    mut event: EventReader<PlayerMovedEvent>,
    mut world: Single<(&GameWorld, &mut ChunkQueue, &mut ChunkCache)>,
    block_reg: Res<RegistryHandle<Block>>,
) {
    let (game_world, mut chunk_queue, mut chunk_cache) = world.into_inner();
    for e in event.read() {
        let old_chunk = chunk::pos_to_chunk_pos(e.old.floor().as_ivec3());
        let new_chunk = chunk::pos_to_chunk_pos(e.new.floor().as_ivec3());
        if old_chunk == new_chunk {
            continue;
        }

        // chunk changed - update what chunks are tracked
        let rad = 5;

        let aabb = Aabb3d {
            min: (new_chunk - rad).as_vec3a(),
            max: (new_chunk + rad).as_vec3a()
        };
        
        let mut to_remove = VecDeque::new();
        {

            for (pos, chunk) in chunk_cache.iter() {
                let point_aabb = Aabb3d {
                    min: pos.as_vec3a(),
                    max: pos.as_vec3a()
                };
                // if intersects, then this chunk is in range of the player. continue.
                if aabb.intersects(&point_aabb) {
                    continue;
                }
                // else let's remove this chunk.
                // TODO: create queue for this as chunks unloading need to be saved to disk
                to_remove.push_back(pos.clone());
            }
        }
        // split to make sure the immutable borrow in iter() is dropped.
        while !to_remove.is_empty() {
            let pos = to_remove.pop_front().unwrap();
            chunk_cache.remove_from_cache(pos, &mut commands);
        }


        // create brand new chunks
        for x in -rad..rad + 1 {
            for z in -rad..rad + 1 {
                for y in -rad..rad + 1 {
                    let coord = ivec3(x, y, z) + new_chunk;
                    if chunk_cache.get_chunk(coord).is_some() {
                        continue;
                    }
                    if chunk_queue.currently_generating.get(&coord).is_some() {
                        continue;
                    }

                    // generate
                    let reg = block_reg.clone();
                    let task = AsyncComputeTaskPool::get().spawn(async move {
                        make_box(reg.as_ref())
                    });
                    chunk_queue.currently_generating.insert(coord, task);

                }
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

    ChunkData::new(vec, palette)

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


    ChunkData::new(vec, palette)
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


    ChunkData::new(vec, palette)

}