use std::collections::HashMap;
use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::image::Image;
use bevy::prelude::{NextState, Res, ResMut, Resource};
use bevy::render::render_resource::{Extent3d, TextureDimension};
use crate::asset::block::{Block, BlockModel};
use crate::core::state::LoadingState;
use crate::registry::block::BlockRegistry;
use crate::render::material::BlockMaterial;

#[derive(Debug, Default, Clone, Resource)]
pub struct BlockTextures {
    map: HashMap<Handle<Image>, u32>,
    pub array_texture: Handle<Image>,
    pub material: Handle<BlockMaterial>,
}

impl BlockTextures {
    pub fn get_texture_id(&self, name: &Handle<Image>) -> u32 {
        self.map[name]
    }
}


// runs once on entering Textures state.
// All of these textures are guaranteed to be loaded
pub fn create_block_array_texture(
    all_blocks: Res<BlockRegistry>,
    mut block_textures: ResMut<BlockTextures>,
    block_asset: Res<Assets<Block>>,
    block_model_asset: Res<Assets<BlockModel>>,
    mut image_asset: ResMut<Assets<Image>>,
    mut next_load_state: ResMut<NextState<LoadingState>>,
    mut materials: ResMut<Assets<BlockMaterial>>,
) {
    let mut i = 0_u32;

    let mut size = None;
    let mut format = None;
    let mut new_data = Vec::new();


    for (k, v) in all_blocks.iter() {

        let texture_handle = (
            &block_model_asset.get(
                &block_asset.get(v).unwrap().model_handle
            ).unwrap().texture_handle
        ).clone();

        let image = image_asset.get(&texture_handle).unwrap();
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


        block_textures.map.insert(texture_handle, i);

        i += 1;
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