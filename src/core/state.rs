use bevy::prelude::States;

#[derive(States, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum MainGameState {
    #[default]
    LoadStartup,
    Menu,
    InGame
}


#[derive(States, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum LoadingState {
    #[default]
    Assets,
    Registries,
    Textures,
    BlockCache,
    Done,
}