use std::any::TypeId;
use bevy::asset::{ron, LoadedFolder};
use bevy::prelude::*;
use crate::asset::block::{Block, BlockLoader, BlockModel, BlockModelLoader};
use crate::asset::procedural::BlockTextures;
use crate::core::state::LoadingState;

pub mod material;
pub mod block;
pub mod procedural;

// plugin that handles creating array textures and texture atlases
#[derive(Default)]
pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_asset::<Block>()
            .init_asset::<BlockModel>()
            .init_asset_loader::<BlockLoader>()
            .init_asset_loader::<BlockModelLoader>()
            .init_resource::<BlockTextures>()
            .add_systems(OnEnter(LoadingState::Textures), procedural::create_block_array_texture)
        ;
    }
}



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
