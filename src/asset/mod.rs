use crate::asset::block::{BlockAsset, BlockLoader, BlockModelAsset, BlockModelLoader};
use bevy::asset::{ron, LoadedFolder};
use bevy::prelude::*;
use std::any::TypeId;

pub mod block;

/// Plugin that handles loading assets using Bevy's Asset system. 
/// Some of the assets loaded are converted into other data structures 
/// that are slimmer and contain the info they need only (e.g. BlockAsset -> Block, BlockModelAsset -> BlockModelMinimal).
#[derive(Default)]
pub struct GameAssetPlugin;

impl Plugin for GameAssetPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_asset::<BlockAsset>()
            .init_asset::<BlockModelAsset>()
            .init_asset_loader::<BlockLoader>()
            .init_asset_loader::<BlockModelLoader>()
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
    /// invalid ron, for whatever reason.
    #[error("Invalid RON: {0}")]
    InvalidRon(String)
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
