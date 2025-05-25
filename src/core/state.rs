use bevy::prelude::States;

#[derive(States, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum MainGameState {
    #[default]
    LoadStartup,
    InGame
}


#[derive(States, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum LoadingState {
    #[default]
    Registries,
    Textures,
    BlockCache,
    Done,
}