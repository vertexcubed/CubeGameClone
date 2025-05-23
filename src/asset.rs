use bevy::asset::{ron, LoadedFolder};
use bevy::prelude::*;
use std::any::TypeId;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::render::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError};

#[derive(Debug, thiserror::Error)]
pub enum AssetLoaderError {
    /// An [IO](std::io) Error
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    /// A [RON](ron) Error
    #[error("Could not parse RON: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),
}


// destructures a loaded folder into its asset handles.
pub fn get_handles_in<T: Asset>(
    folder: &LoadedFolder,
) -> Vec<Handle<T>> {
    // code by @coderdude1 in the Bevy discord. 
    // https://discord.com/channels/691052431525675048/1326990470152323072/1326996564383895687
    
    let type_id = TypeId::of::<T>();

    folder.handles
        .clone()
        .into_iter()
        .filter_map(|h| {
            if h.type_id() == type_id { Some(h.typed::<T>()) }
            else { None }
        })
        .collect::<Vec<_>>()
}

const SHADER_ASSET_PATH: &str = "shader/block.wgsl";


#[derive(Debug, Clone, Asset, TypePath, AsBindGroup)]
pub struct BlockMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub array_texture: Handle<Image>,
}
impl BlockMaterial {
    pub const ATTRIBUTE_ARRAY_ID: MeshVertexAttribute =
        MeshVertexAttribute::new("ArrayId", 988540917, VertexFormat::Uint32);

}

impl Material for BlockMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_ASSET_PATH.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER_ASSET_PATH.into()
    }

    fn specialize(pipeline: &MaterialPipeline<Self>, descriptor: &mut RenderPipelineDescriptor, layout: &MeshVertexBufferLayoutRef, key: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            BlockMaterial::ATTRIBUTE_ARRAY_ID.at_shader_location(2),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(3)
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())

    }
}