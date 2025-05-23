//#import bevy_pbr::{
//    forward_io::VertexOutput,
//    mesh_view_bindings::view,
//    pbr_types::{STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT, PbrInput, pbr_input_new},
//    pbr_functions as fns,
//    pbr_bindings,
//}
//#import bevy_core_pipeline::tonemapping::tone_mapping

#import bevy_pbr::mesh_functions::{mesh_position_local_to_world, mesh_normal_local_to_world, get_world_from_local}
#import bevy_pbr::view_transformations::position_world_to_clip

//TODO: update to work with PBR?

@group(2) @binding(0) var my_array_texture: texture_2d_array<f32>;
@group(2) @binding(1) var my_array_texture_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) texture_id: u32,
    @location(3) normal: vec3<f32>
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) texture_id: u32,
}

// I really don't know how WGSL works so we winging this shit

@vertex
fn vertex(
    vertex: Vertex
) -> VertexOutput {

    let world_from_local = get_world_from_local(vertex.instance_index);

    var out: VertexOutput;
    out.world_normal = mesh_normal_local_to_world(vertex.normal, vertex.instance_index);

    out.world_position = mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
    out.uv = vertex.uv;
    out.texture_id = vertex.texture_id;

    return out;
}


@fragment
fn fragment(
    mesh: VertexOutput,
) -> @location(0) vec4<f32> {
    var index = mesh.texture_id;
    var col = f32(index) / 3.0;

    // sample a 2d array texture
    return textureSample(my_array_texture, my_array_texture_sampler, mesh.uv, index);

//    return vec4(mesh.uv, col, 1.0);

//    return vec4(1.0, 1.0, 1.0, 1.0);
}