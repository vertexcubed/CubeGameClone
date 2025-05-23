use crate::asset;
use crate::data::block::{AllBlocks, Block};
use crate::errors::RegistryError;
use bevy::asset::{LoadedFolder, RecursiveDependencyLoadState};
use bevy::prelude::*;
use crate::state::{LoadingState, MainGameState};

#[derive(Default)]
pub struct GameInitPlugin;
impl Plugin for GameInitPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(AllBlocks::default())
            .insert_resource(LoadedFolders::default())
            .init_state::<MainGameState>()
            .init_state::<LoadingState>()
            .add_systems(Startup, load_folders)
            .add_systems(Update, (all_folders_loaded, check_loading_blocks)
                .run_if(in_state(LoadingState::Registries))
            )
            .add_systems(OnEnter(LoadingState::Done), finish_loading)
        ;
    }
}

#[derive(Resource, Default)]
struct LoadedFolders {
    blocks: (Handle<LoadedFolder>, bool)
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
    all_blocks: Res<Assets<Block>>,
    mut block_res: ResMut<AllBlocks>,
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
            let block_handles = asset::get_handles_in::<Block>(block_folder);

            if let Err(err) = load_blocks(all_blocks, block_res, block_handles) {
                error!("Error loading blocks: {err}")
            }

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

fn load_blocks(
    block_asset: Res<Assets<Block>>,
    mut block_res: ResMut<AllBlocks>,
    block_vec: Vec<Handle<Block>>
) -> Result<(), RegistryError> {

    for h in block_vec {
        let b = block_asset.get(&h).unwrap();
        block_res.add(b.id.as_str(), h)?;
    }
    block_res.freeze();

    Ok(())
}

// only runs in registry loading state
fn all_folders_loaded(
    loaded_folders: Res<LoadedFolders>,
    mut next_load_state: ResMut<NextState<LoadingState>>,
) {

    if loaded_folders.blocks.1 {
        next_load_state.set(LoadingState::Textures)
    }
}


// runs finally once all loading is done
fn finish_loading(
    mut next_game_state: ResMut<NextState<MainGameState>>,
) {
    next_game_state.set(MainGameState::InGame);
}