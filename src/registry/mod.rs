pub mod block;

use bevy::prelude::*;
use block::BlockRegistry;

#[derive(Default)]
pub struct RegistryPlugin;

impl Plugin for RegistryPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(BlockRegistry::new())
        
        ;
    }
}