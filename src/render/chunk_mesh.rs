use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParamItem;
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_asset::{PrepareAssetError, RenderAsset};
use bevy::render::render_resource::PrimitiveTopology;


/// Custom mesh asset to bypass some of the issues with allocating regular bevy meshes.
///
#[derive(Asset, Debug, Clone, TypePath)]
pub struct ChunkMesh {
    primitive_topology: PrimitiveTopology,
    indices: Indices,
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    normals: Vec<[f32; 3]>,
    texture_indices: Indices,
    pub render_asset_usages: RenderAssetUsages
}
impl ChunkMesh {
    pub fn new(
        primitive_topology: PrimitiveTopology,
        indices: Indices,
        positions: Vec<[f32; 3]>,
        uvs: Vec<[f32; 2]>,
        normals: Vec<[f32; 3]>,
        texture_indices: Indices,
        render_asset_usages: RenderAssetUsages
    ) -> Self {
        Self { primitive_topology, indices, positions, uvs, normals, texture_indices, render_asset_usages }
    }

    pub fn primitive_topology(&self) -> PrimitiveTopology {
        self.primitive_topology
    }
    
}


#[derive(Debug, Clone, TypePath)]
pub struct RenderChunkMesh {
    
}
impl RenderAsset for RenderChunkMesh {
    type SourceAsset = ChunkMesh;
    type Param = ();

    fn prepare_asset(source_asset: Self::SourceAsset, asset_id: AssetId<Self::SourceAsset>, param: &mut SystemParamItem<Self::Param>) -> std::result::Result<Self, PrepareAssetError<Self::SourceAsset>> {
        todo!()
    }
}