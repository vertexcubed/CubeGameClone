mod asset;
mod core;
mod registry;
mod world;
mod render;
mod ui;
mod math;

use crate::registry::RegistryPlugin;
use crate::render::pipeline::GameRenderPipelinePlugin;
use crate::render::GameRenderPlugin;
use crate::ui::GameUiPlugin;
use crate::world::GameWorldPlugin;
use asset::GameAssetPlugin;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::WireframePlugin;
use bevy::prelude::*;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::{RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::window::PresentMode;
use bitvec::field::BitField;
use bitvec::view::BitViewSized;
use core::CoreGamePlugin;
use rand::Rng;

fn main() {

    App::new()
        .add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        // WARN this is a native only feature. It will not work with webgl or webgpu
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    }),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "GTClone".into(),
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                }),
            WireframePlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),


            CoreGamePlugin::default(),
            GameAssetPlugin::default(),
            RegistryPlugin::default(),
            GameWorldPlugin::default(),
            GameRenderPlugin::default(),
            GameRenderPipelinePlugin::default(),
            GameUiPlugin::default(),
        ))

        .run();
}