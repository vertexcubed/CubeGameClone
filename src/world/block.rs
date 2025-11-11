use crate::core::errors::ChunkError::{DuplicateChunk, NotFound};
use crate::core::errors::{BlockStateError, ChunkError, WorldError};
use crate::core::event::SetBlockEvent;
use crate::registry::block::Block;
use crate::registry::{Registry, RegistryHandle};
use crate::render;
use crate::render::block::BlockTextures;
use crate::render::block::MeshDataCache;
use crate::world::chunk::{Chunk, ChunkData, ChunkMarker, ChunkMeshMarker, ChunkNeedsMeshing};
use crate::world::{chunk, noise_gen_function, temp_gen_function};
use bevy::app::PostUpdate;
use bevy::asset::Assets;
use bevy::ecs::system::SystemState;
use bevy::log::info_span;
use bevy::math::{ivec3, Vec3};
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::{error, info, warn, App, Children, Commands, Component, Entity, EventWriter, Events, First, IVec3, IntoScheduleConfigs, Mesh, Mesh3d, PreUpdate, Query, QueryState, Res, ResMut, Single, Visibility, With};
use bevy::tasks::futures_lite::future;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};
use std::collections::hash_map::Iter;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use bevy::camera::primitives::Aabb;
use serde::{Deserialize, Serialize};
use crate::core::errors::BlockStateError::InvalidId;
use crate::world::generation::{SineHeightMap, WorldGenerator};

/// A component that represents a world that can be read/written from. Stores the actual Chunk map,
/// along with information about the world status (i.e. chunk generation status)
#[derive(Component)]
pub struct BlockWorld {
    map: ChunkMap,
    chunk_queue: ChunkQueue,
}



/// A bunch of queues and maps representing chunks that are currently being generated, and chunks that are currently being meshed.
/// Stores the tasks for these jobs too.
#[derive(Debug, Default)]
pub struct ChunkQueue {
    to_generate: VecDeque<IVec3>,
    to_despawn: VecDeque<IVec3>,
    currently_generating: HashMap<IVec3, Task<ChunkData>>,
    finished_generating: VecDeque<(IVec3, ChunkData)>,
    currently_meshing: HashMap<IVec3, Task<Option<Mesh>>>,
    finished_meshing: VecDeque<(IVec3, Option<Mesh>)>,
}



impl BlockWorld {
    pub fn new() -> Self {
        BlockWorld {
            map: ChunkMap::default(),
            chunk_queue: ChunkQueue::default(),
        }
    }

    /// Gets a block at a given Block position.
    /// Note: this creates and discards a `RwLockReadGuard`, which may be slow if doing large amounts of reads. In this case, consider accessing the chunk map directly.
    pub fn get_block(&self, pos: &IVec3) -> Result<BlockState, WorldError> {
        let pos = pos.clone();
        let chunk_pos = chunk::pos_to_chunk_pos(pos);
        let chunk_local = chunk::pos_to_chunk_local(pos);
        let Some(chunk) = self.map.get_chunk(&chunk_pos) else {
            return Err(WorldError::UnloadedChunk(chunk_pos));
        };
        Ok(chunk.get_block(chunk_local)?)
    }

    /// Sets a block at a given Block position.
    /// Note: this creates and discards a `RwLockWriteGuard`, which may be slow if doing large amounts of writes. In this case, consider accessing the chunk map directly.
    pub fn set_block(&mut self, commands: &mut Commands, pos: &IVec3, block: BlockState) -> Result<BlockState, WorldError> {
        let pos = pos.clone();
        let chunk_pos = chunk::pos_to_chunk_pos(pos);
        let chunk_local = chunk::pos_to_chunk_local(pos);
        let Some(chunk) = self.map.get_chunk_mut(&chunk_pos) else {
            return Err(WorldError::UnloadedChunk(chunk_pos));
        };
        let res = chunk.set_block(chunk_local, block.clone())?;

        commands.trigger(SetBlockEvent {
            pos: pos,
            old: res.clone(),
            new: block,
        });

        Ok(res)
    }

    pub fn get_chunk_map(&self) -> &ChunkMap {
        &self.map
    }

    pub fn is_queued_for_generation(&self, pos: &IVec3) -> bool {
        self.chunk_queue.currently_generating.contains_key(pos)
    }

    pub fn is_queued_for_meshing(&self, pos: &IVec3) -> bool {
        if self.chunk_queue.currently_meshing.contains_key(pos) {
            true
        } else {
            for (p, _) in self.chunk_queue.finished_meshing.iter() {
                if *pos == *p {
                    return true;
                }
            }
            false
        }
    }

    pub fn get_chunk_map_mut(&mut self) -> &mut ChunkMap {
        &mut self.map
    }


    pub fn queue_chunk_generation(&mut self, pos: IVec3) {
        self.chunk_queue.to_generate.push_back(pos);
    }
    pub fn queue_chunk_despawn(&mut self, pos: IVec3) {
        self.chunk_queue.to_despawn.push_back(pos);
    }
}









/// Stores entity ids for all chunks currently loaded in the world / in memory.
/// Backed by an Arc, so can be cloned and sent to other threads.
/// All operations will require you to acquire a LockGuard first.
#[derive(Debug)]
pub struct ChunkMap {
    data: HashMap<IVec3, Chunk>
}
impl Default for ChunkMap {
    fn default() -> Self {
        Self {
            data: HashMap::with_capacity(1000)
        }
    }
}

impl ChunkMap {

    // gets the chunk entity at this position. Cheap to clone.
    pub fn get_chunk(&self, pos: &IVec3) -> Option<&Chunk> {
        self.data.get(pos)
    }

    pub fn get_chunk_mut(&mut self, pos: &IVec3) -> Option<&mut Chunk> {
        self.data.get_mut(pos)
    }
    
    pub fn iter(&self) -> Iter<'_, IVec3, Chunk> {
        self.data.iter()
    }

    pub fn add_chunk(&mut self, chunk: Chunk) -> Result<(), ChunkError> {
        let pos = chunk.get_pos();
        if self.data.contains_key(&pos) {
            return Err(DuplicateChunk(pos));
        }
        self.data.insert(pos, chunk);


        Ok(())
    }

    pub fn remove_chunk(&mut self, pos: IVec3) -> Result<Chunk, ChunkError> {
        if !self.data.contains_key(&pos) {
            return Err(NotFound(pos));
        }
        Ok(self.data.remove(&pos).unwrap())
    }
}






// ===================================
// Systems that require private access
// ===================================
pub fn add_systems(app: &mut App) {
    app
        .add_systems(PostUpdate, (process_generate_queue, process_despawn_queue, receive_generated_chunks, insert_chunk_data, queue_mesh_creation).chain())
        .add_systems(PreUpdate, (receive_generated_meshes, upload_meshes))
    ;
}

fn process_generate_queue(
    mut single: Single<(&mut BlockWorld, &mut WorldGenerator)>,
    mut commands: Commands,
    block_reg: Res<RegistryHandle<Block>>
) {
    let mut single = single.into_inner();
    //rust rover not showing me types so gonna specify here
    let (world, generator): (&mut BlockWorld, &mut WorldGenerator) = (single.0.as_mut(), single.1.as_mut());
    let (map, chunk_queue) = (&mut world.map, &mut world.chunk_queue);
    
    
    
    if chunk_queue.to_generate.is_empty() {
        return;
    }

    while !chunk_queue.to_generate.is_empty() {
        let pos = chunk_queue.to_generate.pop_front().unwrap();


        // info!("Generating chunk {pos}");

        // Create chunk entity
        let chunk_entity = commands.spawn((
            ChunkMarker::new(pos),
            chunk::chunk_pos_to_transform(pos),
            Visibility::Visible,
            )).id();

        let chunk = Chunk::new(pos, chunk_entity);

        if let Err(e) = map.add_chunk(chunk) {
            error!("Failed to add chunk: {}", e);
            continue;
        }
        // create chunk generation task

        let reg = block_reg.clone();

        let height_map = generator.borrow_height_map();
        
        
        let task = AsyncComputeTaskPool::get().spawn(async move {
            // make_box(reg.as_ref())
            // temp_gen_function(pos, reg.as_ref())
            noise_gen_function(pos, reg.as_ref(), height_map)
        });

        chunk_queue.currently_generating.insert(pos, task);
    }
}

fn process_despawn_queue(
    mut world: Single<&mut BlockWorld>,
    mut commands: Commands,
) {
    let world = world.as_mut();
    let (map, chunk_queue) = (&mut world.map, &mut world.chunk_queue);

    if chunk_queue.to_despawn.is_empty() {
        return;
    }


    while !chunk_queue.to_despawn.is_empty() {
        let pos = chunk_queue.to_despawn.pop_front().unwrap();
        let old_chunk = match map.remove_chunk(pos) {
            Ok(o) => o,
            Err(e) => {
                error!("Error despawning chunks: {}", e);
                continue;
            }
        };
        commands.entity(old_chunk.get_entity()).despawn();

    }

}



// receives chunks that have finished generating.
fn receive_generated_chunks(
    mut world: Single<&mut BlockWorld>
) {
    let mut chunk_queue = &mut world.chunk_queue;
    let mut finished = VecDeque::new();
    // this needs to be in a separate scope so the first mutable reference can be dropped.
    {
        for (coord, task) in chunk_queue.currently_generating.iter_mut() {
            let Some(data) = block_on(future::poll_once(task)) else {
                continue;
            };
            finished.push_back((coord.clone(), data));
        }
    }

    while !finished.is_empty() {
        let (coord, data) = finished.pop_front().unwrap();
        chunk_queue.currently_generating.remove(&coord);
        chunk_queue.finished_generating.push_back((coord.clone(), data));
    }
}

// Inserts chunk data for chunks that have finished generating, initializing their data.
fn insert_chunk_data(
    mut world: Single<&mut BlockWorld>,
    mut commands: Commands,
) {
    let world = world.as_mut();

    let (map, chunk_queue) = (&mut world.map, &mut world.chunk_queue);

    if chunk_queue.finished_generating.is_empty() {
        return;
    }

    // println!("Inserting {} chunk data.", chunk_queue.finished_generating.len());
    // let mut write_guard = world.map.write_guard();
    while !chunk_queue.finished_generating.is_empty() {
        let (pos, data) = chunk_queue.finished_generating.pop_front().unwrap();

        // info!("Finished generating chunk {pos}, inserting...");


        let Some(chunk) = map.get_chunk_mut(&pos) else {
            error!("Chunk {pos} doesn't exist!");
            continue;
        };
        if let Err(e) = chunk.init_data(data) {
            error!("Error initializing chunk: {e}")
        }

        let _ = info_span!("insert_needs_meshing").entered();
        let entity = chunk.get_entity();
        commands.entity(entity).insert(ChunkNeedsMeshing);
    }
}

fn queue_mesh_creation(
    mut world: Single<&mut BlockWorld>,
    chunks_to_mesh: Query<(Entity, &ChunkMarker), With<ChunkNeedsMeshing>>,
    mut commands: Commands,

    mut mesh_cache: Res<MeshDataCache>,
) {

    if chunks_to_mesh.is_empty() {
        return;
    }
    let world = world.as_mut();
    let (map, chunk_queue) = (&world.map, &mut world.chunk_queue);

    let iter = chunks_to_mesh.iter();

    for (entity, marker) in iter {
        let pos = marker.get_pos();

        // info!("Meshing chunk {pos}...");

        let chunk = map.get_chunk(&pos).expect("Leaked chunk entity found - chunk entity exists, but is not present in chunk map!");

        let north = map.get_chunk(&(pos + ivec3(0, 0, 1)));
        let south = map.get_chunk(&(pos + ivec3(0, 0, -1)));
        let east = map.get_chunk(&(pos + ivec3(1, 0, 0)));
        let west = map.get_chunk(&(pos + ivec3(-1, 0, 0)));
        let up = map.get_chunk(&(pos + ivec3(0, 1, 0)));
        let down = map.get_chunk(&(pos + ivec3(0, -1, 0)));
        if let (Some(north), Some(south), Some(east), Some(west), Some(up), Some(down)) = (north, south, east, west, up, down) {

            // moved into thread
            let cache = mesh_cache.clone();
            if !(
                chunk.is_initialized() &&
                north.is_initialized() &&
                south.is_initialized() &&
                east.is_initialized() &&
                west.is_initialized() &&
                up.is_initialized() &&
                down.is_initialized()
            ) {
                continue;
            }

            let data_arc = chunk.get_data().unwrap();
            let north_arc = north.get_data().unwrap();
            let south_arc = south.get_data().unwrap();
            let east_arc = east.get_data().unwrap();
            let west_arc = west.get_data().unwrap();
            let up_arc = up.get_data().unwrap();
            let down_arc = down.get_data().unwrap();



            let task = AsyncComputeTaskPool::get().spawn(async move {
                // read the data


                let data = data_arc.read().unwrap();
                let north_data = north_arc.read().unwrap();
                let south_data = south_arc.read().unwrap();
                let east_data = east_arc.read().unwrap();
                let west_data = west_arc.read().unwrap();
                let up_data = up_arc.read().unwrap();
                let down_data = down_arc.read().unwrap();
                let neighbors: render::chunk::NeighborData = (
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
                    Some(render::chunk::create_chunk_mesh(&data, &cache, neighbors))
                }

            });
            chunk_queue.currently_meshing.insert(pos, task);

            // info!("Submitted mesh job for {pos}");
            commands.entity(entity).remove::<ChunkNeedsMeshing>();
        }

    }
}


fn receive_generated_meshes(
    mut world: Single<&mut BlockWorld>,
) {
    let mut chunk_queue = &mut world.chunk_queue;

    let mut finished = VecDeque::new();
    {
        for (coord, task) in chunk_queue.currently_meshing.iter_mut() {

            let Some(mesh) = block_on(future::poll_once(task)) else {
                continue;
            };
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


fn upload_meshes(
    mut commands: Commands,
    mut world: Single<&mut BlockWorld>,
    q_children: Query<&Children, With<ChunkMarker>>,
    q_chunk_meshes: Query<&ChunkMeshMarker>,
    mut meshes: ResMut<Assets<Mesh>>,
    block_textures: Res<BlockTextures>,
) {
    let _span = info_span!("upload_meshes").entered();

    let world = world.as_mut();
    let (map, mut chunk_queue) = (&world.map, &mut world.chunk_queue);

    if chunk_queue.finished_meshing.is_empty() {
        return;
    }


    // if !chunk_queue.finished_meshing.is_empty() {
    //     println!("Currently meshing queue size: {}", chunk_queue.currently_meshing.len());
    //     println!("Chunk queue size: {}", chunk_queue.finished_meshing.len());
    // }


    // let mut new_entities = Vec::new();
    let mut hard_process_limit = MIB_PER_FRAME;
    while !chunk_queue.finished_meshing.is_empty() && hard_process_limit > 0 {

        let (coord, Some(mesh)) = chunk_queue.finished_meshing.pop_front().unwrap() else {
            // air - we don't need to make a mesh and can just move on
            continue;
        };

        // info!("Uploading mesh {coord}");


        // println!("Indices: {mesh_size}");

        // println!("Buffer size: {}, vertex size: {}, num vertices: {}", mesh.get_vertex_buffer_size(), mesh.get_vertex_size(), mesh.count_vertices());

        // scales the amount of "work" done by how big this mesh is
        // if the mesh is very big, less meshes will be uploaded this frame.
        let to_sub = mesh.get_vertex_buffer_size();

        // println!("Coord: {}, count: {}", coord, counter.count);

        let Some(chunk) = map.get_chunk(&coord) else {
            warn!("Chunk {coord} no longer exists in Chunk Map, discarding mesh...");
            continue;
        };
        let chunk_entity = chunk.get_entity();
        // let mut component = q_chunks.get_mut(entity).expect("Invalid entity id");



        // create the mesh handle
        let mesh_handle = meshes.add(mesh);

        let mut needs_new_mesh = true;
        // chunk may or may not already have a mesh.
        if let Ok(children) = q_children.get(chunk_entity) {
            //iter over all the children.
            for child in children.iter() {
                // does this child have a mesh?
                if q_chunk_meshes.contains(child.clone()) {
                    commands.entity(child.clone()).insert(Mesh3d(mesh_handle.clone()));
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
                Aabb::from_min_max(Vec3::ZERO, Vec3::splat(ChunkData::CHUNK_SIZE as f32))
            )).id();

            commands.entity(chunk_entity).add_child(child);
        }
        hard_process_limit -= to_sub as i32;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockState {
    block: String,
    state: BTreeMap<String, String>
}

impl BlockState {
    pub fn new(id: &str, block_reg: &Registry<Block>) -> Result<Self, BlockStateError> {
        match block_reg.get(id) {
            Some(block) => {
                Ok(Self {
                    state: block.get_default_state().clone(),
                    block: String::from(id),
                })
            }
            None => {
                Err(InvalidId(String::from(id)))
            }
        }
    }
    
    pub fn with_state(
        id: &str, 
        state: BTreeMap<String, String>, 
        block_reg: &Registry<Block>) -> Result<Self, BlockStateError> {
        match block_reg.get(id) {
            Some(_) => {
                Ok(Self {
                    state,
                    block: String::from(id),
                })
            }
            None => {
                Err(InvalidId(String::from(id)))
            }
        }
    }
    
    pub fn get_id(&self) -> &str {
        self.block.as_str()
    }
    
    pub fn get_state(&self) -> &BTreeMap<String, String> {
        &self.state
    }

    pub fn is_air(&self) -> bool {
        self.block == "air"
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub enum Direction {
    Up,
    Down,
    North,
    South,
    East,
    West
}