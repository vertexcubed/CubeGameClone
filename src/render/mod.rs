use std::collections::HashMap;
use std::sync::Arc;
use arc_swap::ArcSwap;
use bevy::app::{App, Plugin, Startup};
use bevy::asset::{AssetContainer, Assets};
use bevy::color::palettes::basic::WHITE;
use bevy::input::ButtonInput;
use bevy::pbr::MaterialPlugin;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::{Commands, KeyCode, Mesh3d, NextState, OnEnter, Query, Res, ResMut, Resource, Update, Visibility, With, Without};
use bevy::render::mesh::allocator::MeshAllocatorSettings;
use bevy::render::RenderApp;
use bevy::utils::default;
use crate::asset::block::{BlockDef, BlockModel};
use crate::asset::procedural::BlockTextures;
use crate::core::AllBlockDefs;
use crate::core::state::LoadingState;
use crate::registry::block::BlockRegistry;
use crate::render::material::BlockMaterial;
use crate::world::block::BlockState;

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

#[derive(Debug, Clone, Default)]
pub struct BlockModelMinimal {
    pub index: u32
}



// creates an atomic cache of all block model and texture data.
// Needed to send to other threads
fn create_block_data_cache(
    all_block_defs: Res<AllBlockDefs>,
    mut cache: ResMut<MeshDataCache>,
    block_reg: Res<BlockRegistry>,
    block_asset: Res<Assets<BlockDef>>,
    block_model_asset: Res<Assets<BlockModel>>,
    block_textures: Res<BlockTextures>,
    mut next_load: ResMut<NextState<LoadingState>>
) {
    let mut map = HashMap::new();

    for h in all_block_defs.inner.iter() {
        let block = block_asset.get(h).unwrap();
        for def in block.models.iter() {
            let texture = &block_model_asset.get(&def.model_handle).unwrap().texture_handle;
            let array_id = block_textures.get_texture_id(texture);

            let block = block_reg.get_block(block.id.as_str()).unwrap().clone();
            let state = BlockState {
                block,
                state: def.state.clone(),
            };

            map.insert(state, BlockModelMinimal {
                index: array_id
            });
        }

    }
    cache.inner.store(Arc::new(map));
    
    next_load.set(LoadingState::Done);
}