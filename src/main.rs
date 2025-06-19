mod asset;
mod core;
mod registry;
mod world;
mod render;
mod ui;

use render::material::BlockMaterial;
use crate::registry::RegistryPlugin;
use crate::world::GameWorldPlugin;
use asset::GameAssetPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::basic::WHITE;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::{RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bitvec::field::BitField;
use bitvec::prelude::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::BitViewSized;
use core::CoreGamePlugin;
use rand::distr::Uniform;
use rand::Rng;
use world::chunk::{ChunkData, PaletteEntry};
use crate::render::GameRenderPlugin;
use crate::render::pipeline::GameRenderPipelinePlugin;
use crate::ui::GameUiPlugin;

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