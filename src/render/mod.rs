use std::collections::{HashMap, HashSet};
use std::slice::Iter;
use std::sync::Arc;
use arc_swap::ArcSwap;
use bevy::app::{App, Plugin, Startup};
use bevy::asset::{AssetContainer, Assets};
use bevy::color::palettes::basic::WHITE;
use bevy::image::Image;
use bevy::input::ButtonInput;
use bevy::pbr::MaterialPlugin;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::{BevyError, Commands, Handle, KeyCode, Mesh3d, NextState, OnEnter, Query, Res, ResMut, Resource, Update, Vec2, Vec3, Visibility, With, Without};
use bevy::render::mesh::allocator::MeshAllocatorSettings;
use bevy::render::RenderApp;
use bevy::utils::default;
use crate::asset::block::{BlockAsset, BlockModelAsset, BlockModelFace, FaceType};
use crate::asset::procedural::BlockTextures;
use crate::core::AllBlockAssets;
use crate::core::errors::BlockModelError;
use crate::core::errors::BlockModelError::{CircularDependency, InvalidFace, KeyNotFound};
use crate::core::state::LoadingState;
use crate::registry::block::{Block};
use crate::registry::{RegistryHandle, Registry, RegistryObject};
use crate::render::material::BlockMaterial;
use crate::world::block::{BlockState, Direction};

pub mod material;
pub mod pipeline;
mod chunk_mesh;

#[derive(Default)]
pub struct GameRenderPlugin;

impl Plugin for GameRenderPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(
                MaterialPlugin::<BlockMaterial>::default(),
            )
            .insert_resource(WireframeConfig {
                // The global wireframe config enables drawing of wireframes on every mesh,
                // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
                // regardless of the global configuration.
                global: false,
                // Controls the default color of all wireframes. Used as the default color for global wireframes.
                // Can be changed per mesh using the `WireframeColor` component.
                default_color: WHITE.into(),
            })
            .init_resource::<MeshDataCache>()
            .insert_resource(MeshAllocatorSettings {
                ..default()
            })
            .add_systems(Update, toggle_wireframe)
            .add_systems(OnEnter(LoadingState::BlockCache), create_block_data_cache)
        ;
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            // render_app.add_systems(Startup, update_mesh_allocator);
        }
    }
}

fn toggle_wireframe(
    kb_input: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<WireframeConfig>,
    mut to_toggle: Query<&mut Visibility, (With<Mesh3d>, Without<NoWireframe>)>,
) {

    // toggles on and off wireframe
    if kb_input.just_pressed(KeyCode::KeyZ) {
        config.global = !config.global;
        // for mut vis in to_toggle.iter_mut() {
        //     *vis = match config.global {
        //         true => Visibility::Hidden,
        //         false => Visibility::Visible,
        //     }
        // }
    }
}


#[derive(Resource, Debug, Clone, Default)]
pub struct MeshDataCache {
    pub inner: Arc<ArcSwap<HashMap<BlockState, BlockModelMinimal>>>
}

#[derive(Debug, Clone)]
pub struct BlockModelMinimal {
    faces: Vec<FaceMinimal>,
    full_sides: u8,
}
impl BlockModelMinimal {
    pub fn face_iter(&self) -> Iter<'_, FaceMinimal> {
        self.faces.iter()
    }

    // inner func. includes a set of visited models to track circular dependencies
    fn from_asset_rec(
        model_handle: &Handle<BlockModelAsset>, 
        block_model_asset: &Assets<BlockModelAsset>, 
        block_textures: &BlockTextures, 
        visited_models: &mut HashSet<Handle<BlockModelAsset>>,
        texture_map: &mut HashMap<String, (Handle<Image>, String)>
    ) -> Result<BlockModelMinimal, BlockModelError> {
        let model = block_model_asset.get(model_handle).unwrap();
        
        println!("Texture map: {:?}", texture_map);
        
        
        // add to texture map.
        for (k, v) in model.texture_handles.iter() {
            if texture_map.contains_key(k) {
                continue;
            }
            texture_map.insert(k.clone(), (v.clone(), model.textures.get(k).unwrap().clone()));
        }
        let mut face_vec = vec![];
        let mut full_sides: u8 = 0;
        // recurse on parent
        if let Some(parent) = &model.parent_handle {
            // if true, circular dependency, cannot create model.
            if visited_models.contains(parent) {
                let parent_str = (&model.parent.as_ref().unwrap().clone()).clone();
                return Err(CircularDependency(parent_str));
            }
            let rec = Self::from_asset_rec(parent, block_model_asset, block_textures, visited_models, texture_map)?;
            (face_vec, full_sides) = (rec.faces, rec.full_sides);
        }

        
        
        
        // construct new faces
        for face in model.faces.iter() {
            // chcek if face texture is OK or not
            let texture_handle = texture_map.get(&face.texture);
            if texture_handle.is_none() {
                return Err(KeyNotFound(face.texture.clone()).into());
            }
            let array_id = block_textures.get_texture_id(&texture_handle.unwrap().0.clone());
            if array_id.is_none() {
                let texture_path = texture_map.get(&face.texture).unwrap().1.clone();
                return Err(KeyNotFound(texture_path).into());
            }
            let face = FaceMinimal::from_asset(face, array_id.unwrap())?;
            face_vec.push(face);
        }
        
        // maps to what sides are full, and can be culled. 
        // So if we're trying to cull the up side of Block A,
        // the down side of Block B must be true for that bit.

        // if we've defined sides to cull, wipe the parent's value.
        if !model.full_sides.is_empty() {
            full_sides = 0;
        }
        
        for side in model.full_sides.iter() {
            match side {
                Direction::Up => {
                    full_sides = full_sides | (0b1)
                }
                Direction::Down => {
                    full_sides = full_sides | (0b1 << 1)
                }
                Direction::North => {
                    full_sides = full_sides | (0b1 << 2)
                }
                Direction::South => {
                    full_sides = full_sides | (0b1 << 3)
                }
                Direction::East => {
                    full_sides = full_sides | (0b1 << 4)
                }
                Direction::West => {
                    full_sides = full_sides | (0b1 << 5)
                }
            }
        }


        Ok(BlockModelMinimal {
            faces: face_vec,
            full_sides
        })
    }

    pub fn from_asset(model: &Handle<BlockModelAsset>, block_model_asset: &Assets<BlockModelAsset>, block_textures: &BlockTextures) -> Result<Self, BlockModelError> {
        let mut set = HashSet::new();
        set.insert(model.clone());
        Self::from_asset_rec(model, block_model_asset, block_textures, &mut set, &mut HashMap::new())
    }
    
    pub fn is_full(&self, direction: Direction) -> bool {
        match direction {
            Direction::Up => {
                self.full_sides & (0b1) != 0
            }
            Direction::Down => {
                self.full_sides & (0b1 << 1) != 0
            }
            Direction::North => {
                self.full_sides & (0b1 << 2) != 0
            }
            Direction::South => {
                self.full_sides & (0b1 << 3) != 0
            }
            Direction::East => {
                self.full_sides & (0b1 << 4) != 0
            }
            Direction::West => {
                self.full_sides & (0b1 << 5) != 0
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FaceMinimal {
    vertices: Vec<Vertex>,
    normal: Vec3,
    indices: Vec<u32>,
    texture_index: u32,
    cull_mode: Option<Direction>,
}
impl FaceMinimal {
    pub fn from_asset(
        face: &BlockModelFace,
        texture_id: u32,
    ) -> Result<Self, BlockModelError> {
        let mut vertices = vec![];
        let mut indices = vec![];
        match face.face_type {
            FaceType::Quad => {
                if face.vertices.len() % 4 != 0 {
                    //todo: make Err type
                    let message = format!("Expected multiple of 4 vertices for face type quad, but found {} vertices", face.vertices.len());
                    return Err(InvalidFace(message));
                }
                vertices = face.vertices
                    .iter()
                    .map(|v|
                        Vertex { position: v.pos, uv0: v.uv }
                    )
                    .collect::<Vec<Vertex>>();
                for i in 0..face.vertices.len() / 4 {
                    let mut one_quad = vec![
                        i as u32, i as u32 + 1, i as u32 + 2, i as u32, i as u32 + 2, i as u32 + 3
                    ];
                    indices.append(&mut one_quad);
                }
            }
            FaceType::Triangle => {
                if face.vertices.len() % 3 != 0 {
                    //todo: make Err type
                    let message = format!("Expected multiple of 3 vertices for face type triangle, but found {} vertices", face.vertices.len());
                    return Err(InvalidFace(message));
                }
                vertices = face.vertices
                    .iter()
                    .map(|v|
                        Vertex { position: v.pos, uv0: v.uv }
                    )
                    .collect::<Vec<Vertex>>();
                indices = (0..vertices.len())
                    .map(|i| i as u32)
                    .collect::<Vec<u32>>();
            }
        }
        Ok(Self {
            vertices,
            indices,
            normal: face.normal,
            texture_index: texture_id,
            cull_mode: face.cull_mode
        })
    }

    pub fn get_cull_mode(&self) -> Option<Direction> {
        self.cull_mode
    }

    /// Constructs a tuple for mesh creation of (position, uv0, normal, indices, texture_indices)
    pub fn get_face_data(&self, chunk_pos: Vec3, index_offset: u32) -> (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
        let mut pos = vec![];
        let mut uv0 = vec![];
        let mut normal = vec![];
        let indices = self.indices.iter().map(|n| n + index_offset).collect::<Vec<u32>>();
        let mut texture_indices = vec![];
        for vertex in self.vertices.iter() {
            pos.push([chunk_pos.x + vertex.position.x, chunk_pos.y + vertex.position.y, chunk_pos.z + vertex.position.z]);
            uv0.push([vertex.uv0.x, vertex.uv0.y]);
            normal.push([self.normal.x, self.normal.y, self.normal.z]);
            texture_indices.push(self.texture_index);
        }
        (pos, uv0, normal, indices, texture_indices)
    }
}


#[derive(Debug, Clone)]
struct Vertex {
    position: Vec3,
    uv0: Vec2,
}

// creates an atomic cache of all block model and texture data.
// Needed to send to other threads
fn create_block_data_cache(
    all_block_defs: Res<AllBlockAssets>,
    mut cache: ResMut<MeshDataCache>,
    block_reg: Res<RegistryHandle<Block>>,
    block_asset: Res<Assets<BlockAsset>>,
    block_model_asset: Res<Assets<BlockModelAsset>>,
    block_textures: Res<BlockTextures>,
    mut next_load: ResMut<NextState<LoadingState>>
) -> Result<(), BevyError> {
    let mut map: HashMap<BlockState, BlockModelMinimal> = HashMap::new();
    let reg = block_reg.as_ref().as_ref();

    // stores a maps of block model handles to blockstates, the blockstate being the FIRST blockstate to use this exact model handle.
    // Done like this to avoid mutable and immutable borrows at the same time
    let mut models: HashMap<Handle<BlockModelAsset>, BlockState> = HashMap::new();

    for h in all_block_defs.inner.iter() {
        let block = block_asset.get(h).unwrap();
        for def in block.models.iter() {
            // TODO: add support for parents + overrides
            let block = reg.get(block.id.as_str()).unwrap();
            let state = BlockState::with_state(block.get_id(), def.state.clone(), &block_reg)?;

            let model = block_model_asset.get(&def.model_handle).unwrap();

            // if we've already visited this model, clone it and skip
            if models.contains_key(&def.model_handle) {
                map.insert(state, map.get(models.get(&def.model_handle).unwrap()).unwrap().clone());
                continue;
            }
            let minimal = BlockModelMinimal::from_asset(&def.model_handle, &block_model_asset, &block_textures)?;

            map.insert(state.clone(), minimal);
            // cloning blockstates are cheap
            models.insert(def.model_handle.clone(), state);
        }

    }
    cache.inner.store(Arc::new(map));
    
    next_load.set(LoadingState::Done);

    Ok(())
}