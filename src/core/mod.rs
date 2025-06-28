use crate::asset::block::BlockAsset;
use crate::core::errors::RegistryError;
use crate::core::event::{JoinedWorldEvent, PlayerMovedEvent, SetBlockEvent};
use crate::core::state::{LoadingState, MainGameState};
use crate::registry::block::Block;
use crate::registry::{Registry, RegistryHandle};
use crate::world::camera::MainCamera;
use crate::world::block::BlockWorld;
use crate::{asset, registry};
use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::{AssetServer, Assets, Handle, LoadedFolder, RecursiveDependencyLoadState};
use bevy::log::error;
use bevy::prelude::*;
use std::sync::Arc;

#[allow(dead_code)]
pub mod state;
#[allow(dead_code)]
pub mod errors;
#[allow(dead_code)]
pub mod event;

/// Core plugin that registers states, events, core systems, etc.
#[derive(Default)]
pub struct CoreGamePlugin;

impl Plugin for CoreGamePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(LoadedFolders::default())
            .init_resource::<AllBlockAssets>()
            .init_state::<MainGameState>()
            .init_state::<LoadingState>()
            .add_event::<PlayerMovedEvent>()
            
            .add_systems(Startup, load_folders)
            .add_systems(Update, (all_folders_loaded, check_loading_blocks)
                .run_if(in_state(LoadingState::Assets))
            )
            .add_systems(OnEnter(LoadingState::Done), finish_loading)
        ;
    }
}

#[derive(Resource, Default)]
struct LoadedFolders {
    blocks: (Handle<LoadedFolder>, bool)
}

#[derive(Resource)]
pub struct AllBlockAssets {
    pub inner: Vec<Handle<BlockAsset>>
}
impl Default for AllBlockAssets {
    fn default() -> Self {
        Self {
            inner: Vec::new()
        }
    }
}



// runs on startup
fn load_folders(
    asset_server: Res<AssetServer>,
    mut loaded_folders: ResMut<LoadedFolders>,
) {
    loaded_folders.blocks = (asset_server.load_folder("block"), false);
}

// runs during registry loading
fn check_loading_blocks(
    asset_server: Res<AssetServer>,
    all_folders: Res<Assets<LoadedFolder>>,
    mut loaded_folders: ResMut<LoadedFolders>,
    mut def_list: ResMut<AllBlockAssets>
) {

    let (folder_handle, already_loaded) = &loaded_folders.blocks;
    if *already_loaded {
        return;
    }

    let block_folder = all_folders.get(folder_handle);
    if block_folder.is_none() {
        return;
    }
    let block_folder = block_folder.unwrap();
    match asset_server.get_recursive_dependency_load_state(folder_handle) {
        Some(RecursiveDependencyLoadState::Loaded) => {

            // we've loaded all blocks, yay! We can safely unwrap these
            let block_handles = asset::get_handles_in::<BlockAsset>(block_folder);
            def_list.inner = block_handles;

            // if let Err(err) = registry::block::load_blocks(block_asset, block_reg, def_list.into()) {
            //     error!("Error loading blocks: {err}")
            // }

            loaded_folders.blocks.1 = true;
        }
        Some(RecursiveDependencyLoadState::Failed(err)) => {
            error!("Error loading blocks: {err}");
            loaded_folders.blocks.1 = true;

        }
        // none case, or loading/notloaded
        _ => { }
    }
    //done
}





// only runs in registry loading state
fn all_folders_loaded(
    loaded_folders: Res<LoadedFolders>,
    mut next_load_state: ResMut<NextState<LoadingState>>,
) {

    if loaded_folders.blocks.1 {
        next_load_state.set(LoadingState::Registries)
    }
}








// runs finally once all loading is done
fn finish_loading(
    mut next_game_state: ResMut<NextState<MainGameState>>,
) {
    // info!("Finished loading.");
    next_game_state.set(MainGameState::InGame);
}