use crate::asset::block::{BlockAsset, BlockModelAsset};
use crate::core::state::LoadingState;
use crate::core::AllBlockAssets;
use crate::registry::block::Block;
use crate::registry::{RegistryHandle, RegistryObject};
use crate::render::material::BlockMaterial;
use crate::world::block::BlockState;
use bevy::app::{App, Plugin};
use bevy::asset::{AssetContainer, Assets, RenderAssetUsages};
use bevy::color::palettes::basic::WHITE;
use bevy::image::Image;
use bevy::input::ButtonInput;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig};
use bevy::pbr::MaterialPlugin;
use bevy::prelude::{info, BevyError, Gizmos, Handle, KeyCode, Mesh3d, NextState, OnEnter, Query, Res, ResMut, Resource, Transform, Update, Visibility, With, Without};
use bevy::render::mesh::allocator::MeshAllocatorSettings;
use bevy::render::render_resource::{Extent3d, TextureDimension};
use bevy::render::RenderApp;
use bevy::utils::default;
use block::BlockTextures;
use block::{BlockModelMinimal, MeshDataCache};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use bevy::color::palettes::css;
use bevy::math::Vec3;
use crate::math::block::BlockPos;
use crate::world::LookAtData;

pub mod material;
pub mod pipeline;
pub mod block;
pub mod chunk;

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
            .init_resource::<BlockTextures>()
            .init_resource::<MeshDataCache>()
            .insert_resource(MeshAllocatorSettings {
                ..default()
            })
            .add_systems(Update, (toggle_wireframe, render_look_at_outline))
            .add_systems(OnEnter(LoadingState::BlockCache), create_block_data_cache)
            .add_systems(OnEnter(LoadingState::Textures), create_block_array_texture)
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

    // info!("Creating block data cache.");


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
    cache.inner = Arc::new(map);
    next_load.set(LoadingState::Done);
    Ok(())
}

// runs once on entering Textures state.
// All of these textures are guaranteed to be loaded
fn create_block_array_texture(
    all_block_defs: Res<AllBlockAssets>,
    mut block_textures: ResMut<BlockTextures>,
    block_asset: Res<Assets<BlockAsset>>,
    block_model_asset: Res<Assets<BlockModelAsset>>,
    mut image_asset: ResMut<Assets<Image>>,
    mut next_load_state: ResMut<NextState<LoadingState>>,
    mut materials: ResMut<Assets<BlockMaterial>>,
) {

    // info!("Creating block array textures.");

    let mut i = 0_u32;

    let mut size = None;
    let mut format = None;
    let mut new_data = Vec::new();


    let mut visited_models = HashSet::new();
    let mut visited_textures = HashSet::new();
    for h in all_block_defs.inner.iter() {

        for model in block_asset.get(h).unwrap().models.iter() {
            // if we've already visited this model handle? continue on.
            if visited_models.contains(&model.model_handle) {
                continue;
            }
            visited_models.insert(model.model_handle.clone());

            let model =
                block_model_asset.get(
                    &model.model_handle
                ).unwrap();

            for (k, texture_handle) in model.texture_handles.iter() {
                // if we've already added this texture to the array texture? continue on.
                if visited_textures.contains(texture_handle) {
                    continue;
                }
                visited_textures.insert(texture_handle.clone());


                let image = image_asset.get(texture_handle).unwrap();
                let descriptor = &image.texture_descriptor;
                let mut should_convert = false;
                match (size, format) {
                    (None, None) => {
                        size = Some(descriptor.size);
                        format = Some(descriptor.format);
                    }
                    (Some(s), Some(f)) => {
                        if descriptor.size != s {
                            panic!("Block array texture requires size {:?}, but texture {:?} has size {:?}",
                                   s,
                                   descriptor.label,
                                   descriptor.size
                            );
                        }
                        if descriptor.format != f {
                            should_convert = true;
                        }
                    }
                    _ => {
                        panic!("Dead branch");
                    }
                }

                // get around dropped references and stuff
                let data = if should_convert {
                    &image.convert(format.unwrap()).expect("Valid texture format.").data
                } else {
                    &image.data
                };



                match data {
                    None => { panic!("Should not happen")}
                    Some(d) => {
                        for p in d.iter() {
                            new_data.push(*p);
                        }
                    }
                }


                block_textures.map.insert(texture_handle.clone(), i);

                i += 1;
            }

        }
    }

    if visited_textures.len() == 0 {
        panic!("Cannot create Array texture for zero textures.")
    }

    let size = Extent3d {
        width: size.unwrap().width,
        height: size.unwrap().height,
        depth_or_array_layers: i
    };

    let new_image = Image::new(
        size,
        TextureDimension::D2,
        new_data,
        format.unwrap(),
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD
    );

    block_textures.array_texture = image_asset.add(new_image);
    block_textures.material = materials.add(BlockMaterial {
        array_texture: block_textures.array_texture.clone(),
    });
    next_load_state.set(LoadingState::BlockCache);


}




fn render_look_at_outline(
    look_info: Query<&LookAtData>,
    mut gizmos: Gizmos,
) {
    for look in look_info.iter() {
        let Some(pos) = look.look_pos else {
            continue;
        };
        gizmos.cuboid(Transform::from_translation(pos.center()).with_scale(Vec3::splat(1.0)), css::BLACK);
    }
}