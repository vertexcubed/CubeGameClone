use bevy::prelude::*;
use bevy::render::RenderApp;

#[derive(Debug, Default)]
pub struct GameRenderPipelinePlugin;
impl Plugin for GameRenderPipelinePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
        
        
        ;
    }
}


