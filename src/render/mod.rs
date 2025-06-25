use std::collections::HashMap;
use std::sync::Arc;
use bevy::app::{App, Plugin};
use bevy::asset::{AssetContainer, Assets};
use bevy::color::palettes::basic::WHITE;
use bevy::input::ButtonInput;
use bevy::pbr::MaterialPlugin;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::prelude::{BevyError, Handle, KeyCode, Mesh3d, NextState, OnEnter, Query, Res, ResMut, Resource, Update, Visibility, With, Without};
use bevy::render::mesh::allocator::MeshAllocatorSettings;
use bevy::render::RenderApp;
use bevy::utils::default;
use block::{BlockModelMinimal, MeshDataCache};
use crate::asset::block::{BlockAsset, BlockModelAsset};
use crate::asset::procedural::BlockTextures;
use crate::core::AllBlockAssets;
use crate::core::state::LoadingState;
use crate::registry::block::Block;
use crate::registry::{RegistryHandle, RegistryObject};
use crate::render::material::BlockMaterial;
use crate::world::block::BlockState;

pub mod material;
pub mod pipeline;
mod chunk_mesh;
pub mod block;

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