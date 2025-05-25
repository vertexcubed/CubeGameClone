mod asset;
mod core;
mod registry;
mod world;
mod render;

use render::material::BlockMaterial;
use crate::registry::RegistryPlugin;
use crate::world::WorldPlugin;
use asset::AssetPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::basic::WHITE;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::{RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bitvec::field::BitField;
use bitvec::prelude::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::BitViewSized;
use core::CoreGamePlugin;
use rand::distr::Uniform;
use rand::Rng;
use world::chunk::{ChunkData, PaletteEntry};
use crate::render::GameRenderPlugin;

#[derive(Resource)]
struct TestChunk {
    inner: ChunkData
}
impl TestChunk {
    fn new() -> Self {
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

        
        let mut data = ChunkData::new(vec, palette);
        data.add_palette(PaletteEntry::new("diamond_ore"));
        
        TestChunk {
            inner: data
        }
    }
}


fn main() {

    App::new()
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        // WARN this is a native only feature. It will not work with webgl or webgpu
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }),
                ..default()
            }),
            WireframePlugin::default(),
            CoreGamePlugin::default(),
            AssetPlugin::default(),
            RegistryPlugin::default(),
            WorldPlugin::default(),
            GameRenderPlugin::default(),
        ))
        .insert_resource(TestChunk::new())

        .run();
}



#[deprecated]
fn _create_cube() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                // top (facing towards +y)
                [0., 1., 0.],
                [1., 1., 0.],
                [1., 1., 1.],
                [0., 1., 1.],
                // bottom   (-y)
                [0., 0., 0.],
                [1., 0., 0.],
                [1., 0., 1.],
                [0., 0., 1.],
                // right    (+x)
                [1., 0., 0.],
                [1., 0., 1.],
                [1., 1., 1.], // This vertex is at the same position as vertex with index 2, but they'll have different UV and normal
                [1., 1., 0.],
                // left     (-x)
                [0., 0., 0.],
                [0., 0., 1.],
                [0., 1., 1.],
                [0., 1., 0.],
                // back     (+z)
                [0., 0., 1.],
                [0., 1., 1.],
                [1., 1., 1.],
                [1., 0., 1.],
                // forward  (-z)
                [0., 0., 0.],
                [0., 1., 0.],
                [1., 1., 0.],
                [1., 0., 0.],
            ]
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![
                // Assigning the UV coords for the top side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],
                // Assigning the UV coords for the bottom side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],
                // Assigning the UV coords for the right side.
                [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0],
                // Assigning the UV coords for the left side.
                [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0],
                // Assigning the UV coords for the back side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],
                // Assigning the UV coords for the forward side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],

            ]
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![
                // Normals for the top side (towards +y)
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            // Normals for the bottom side (towards -y)
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                // Normals for the right side (towards +x)
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                // Normals for the left side (towards -x)
                [-1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                // Normals for the back side (towards +z)
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                // Normals for the forward side (towards -z)
                [0.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
            ],
        )
        .with_inserted_indices(Indices::U32(vec![
            0,3,1 , 1,3,2, // triangles making up the top (+y) facing side.
            4,5,7 , 5,6,7, // bottom (-y) 0, 1, 3, 1, 2, 3,
            8,11,9 , 9,11,10, // right (+x) 0, 3, 1, 1, 3, 2,
            12,13,15 , 13,14,15, // left (-x) 0, 1, 3, 1, 2, 3,
            16,19,17 , 17,19,18, // back (+z) 0, 3, 1, 1, 3, 2,
            20,21,23 , 21,22,23, // forward (-z) 0, 1, 3, 1, 2, 3,
        ]))
}