use std::slice::Iter;
use bevy::asset::{Assets, Handle};
use std::collections::{HashMap, HashSet};
use bevy::image::Image;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::Resource;
use std::sync::Arc;
use arc_swap::ArcSwap;
use crate::asset::block::{BlockModelAsset, BlockModelFace, FaceType};
use crate::core::errors::BlockModelError;
use crate::core::errors::BlockModelError::{CircularDependency, InvalidFace, KeyNotFound};
use crate::render::material::BlockMaterial;
use crate::world::block::{BlockState, Direction};

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

#[derive(Resource, Debug, Clone, Default)]
pub struct MeshDataCache {
    pub inner: Arc<ArcSwap<HashMap<BlockState, BlockModelMinimal>>>
}

#[derive(Debug, Default, Clone, Resource)]
pub struct BlockTextures {
    pub map: HashMap<Handle<Image>, u32>,
    pub array_texture: Handle<Image>,
    pub material: Handle<BlockMaterial>,
}

impl BlockTextures {
    pub fn get_texture_id(&self, name: &Handle<Image>) -> Option<u32> {
        self.map.get(name).cloned()
    }
}