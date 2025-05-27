use std::collections::HashMap;
use std::sync::Arc;
use arc_swap::ArcSwap;
use bevy::app::{App, Plugin, Startup};
use bevy::asset::{AssetContainer, Assets};
use bevy::color::palettes::basic::WHITE;
use bevy::input::ButtonInput;
use bevy::pbr::MaterialPlugin;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::{KeyCode, Mesh3d, NextState, OnEnter, Query, Res, ResMut, Resource, Update, Visibility, With, Without};
use bevy::render::mesh::allocator::MeshAllocatorSettings;
use bevy::render::RenderApp;
use crate::asset::block::{Block, BlockModel};
use crate::asset::procedural::BlockTextures;
use crate::core::state::LoadingState;
use crate::registry::block::BlockRegistry;
use crate::render::material::BlockMaterial;

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
            .add_systems(Update, toggle_wireframe)
            .add_systems(OnEnter(LoadingState::BlockCache), create_block_data_cache)
        ;
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Startup, update_mesh_allocator);
        }
    }
}


fn update_mesh_allocator(
    mut mesh_allocator_settings: ResMut<MeshAllocatorSettings>,
) {
    mesh_allocator_settings.growth_factor = 3.0;
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
    pub inner: Arc<ArcSwap<HashMap<String, BlockModelMinimal>>>
}

#[derive(Debug, Clone, Default)]
pub struct BlockModelMinimal {
    pub index: u32
}



// creates an atomic cache of all block model and texture data.
// Needed to send to other threads
fn create_block_data_cache(
    mut cache: ResMut<MeshDataCache>,
    block_reg: Res<BlockRegistry>,
    block_asset: Res<Assets<Block>>,
    block_model_asset: Res<Assets<BlockModel>>,
    block_textures: Res<BlockTextures>,
    mut next_load: ResMut<NextState<LoadingState>>
) {
    let mut map = HashMap::new();

    for (k, v) in block_reg.iter() {
        let block = block_asset.get(v).unwrap();
        let texture = &block_model_asset.get(&block.model_handle).unwrap().texture_handle;
        let array_id = block_textures.get_texture_id(texture);
        map.insert(k.clone(), BlockModelMinimal {
            index: array_id
        });
    }
    cache.inner.store(Arc::new(map));

    next_load.set(LoadingState::Done);
}