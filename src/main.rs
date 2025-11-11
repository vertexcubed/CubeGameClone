mod asset;
mod core;
mod registry;
mod world;
mod render;
mod ui;

#[allow(dead_code)]
mod math;

use std::fmt::Formatter;
use std::path;
use std::path::PathBuf;
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
use clap::Parser;
use directories::ProjectDirs;
use core::CoreGamePlugin;




pub const ORG_NAME: &'static str = "vertexcubed";
pub const APP_NAME: &'static str = "GTClone";

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct CliConfigRaw {

    /// Sets the output directory for game save data.
    /// Defaults to $XDG_DATA_HOME / %APPDATA%
    #[arg(short, long, value_name = "FOLDER")]
    output: Option<PathBuf>
}

#[derive(Resource, Debug)]
pub struct RunConfig {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub config_dir: PathBuf,
}
impl From<CliConfigRaw> for RunConfig {
    fn from(value: CliConfigRaw) -> Self {

        let project_dirs = ProjectDirs::from("", ORG_NAME, APP_NAME).expect("Could not find home directory!");

        let (data_dir, cache_dir, config_dir) = match value.output {
            None => {
                (PathBuf::from(project_dirs.data_dir()), PathBuf::from(project_dirs.cache_dir()), PathBuf::from(project_dirs.config_dir()))
            }
            Some(old) => {
                let base = if old.is_absolute() { old } else {
                    path::absolute(old).expect("Error: could not access provided file path.")
                };
                (base.join("data"), base.join("cache"), base.join("config"))
            }
        };
        Self {
            data_dir,
            cache_dir,
            config_dir
        }
    }
}
impl std::fmt::Display for RunConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,
               "{{ Data directory: {:?}, Cache directory: {:?}, Config directory: {:?} }}",
               self.data_dir, self.cache_dir, self.config_dir)
    }
}

fn main() {

    let run_config: RunConfig = CliConfigRaw::parse().into();
    println!("{}", run_config);

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
       .insert_resource(run_config)

        .run();
}