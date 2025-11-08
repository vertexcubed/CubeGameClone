use bevy::asset::{ron, LoadedFolder};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use std::any::TypeId;
use bevy::shader::ShaderRef;

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

    fn specialize(pipeline: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, layout: &MeshVertexBufferLayoutRef, key: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
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