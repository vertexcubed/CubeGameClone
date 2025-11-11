use std::fs;
use crate::asset::block::BlockAsset;
use crate::core::errors::RegistryError;
use crate::core::event::{JoinedWorldEvent, PlayerMovedEvent, SetBlockEvent};
use crate::core::state::{LoadingState, MainGameState};
use crate::registry::block::Block;
use crate::registry::{Registry, RegistryHandle};
use crate::world::camera::MainCamera;
use crate::world::block::BlockWorld;
use crate::{asset, registry, RunConfig};
use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::{ron, AssetServer, Assets, Handle, LoadedFolder, RecursiveDependencyLoadState};
use bevy::log::error;
use bevy::prelude::*;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

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
            
            .add_systems(Startup, load_folders)
            .add_systems(Startup, gen_folders_if_empty)
            .add_systems(Update, (all_folders_loaded, check_loading_blocks)
                .run_if(in_state(LoadingState::Assets))
            )
            .add_systems(OnEnter(LoadingState::Done), finish_loading)
            .add_systems(OnEnter(LoadingState::Done), test_writing_to_disk)
        ;
    }
}


fn gen_folders_if_empty(run_config: Res<RunConfig>) -> Result<(), BevyError> {
    println!("Generating output folders...");
    fs::create_dir_all(&run_config.data_dir)?;
    fs::create_dir_all(&run_config.config_dir)?;
    fs::create_dir_all(&run_config.cache_dir)?;

    Ok(())
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
    info!("Finished loading.");
    next_game_state.set(MainGameState::InGame);
}


#[derive(Serialize, Deserialize, Debug)]
struct MyStruct {
    name: String,
    vec: Vec<u8>,
    other: bool
}




fn test_writing_to_disk(
    run_config: Res<RunConfig>
) -> Result<(), BevyError> {
    let my_struct = MyStruct {
        name: String::from("Meow"),
        vec: vec![0, 12, 33, 14, 0, 0, 0, 2, 3, 4, 5],
        other: true
    };
    let data = ron::ser::to_string(&my_struct)?;
    let file_name = "my_cool_file.ron";
    fs::write(run_config.data_dir.join(file_name), data.as_bytes())?;
    println!("Meow");

    Ok(())
}