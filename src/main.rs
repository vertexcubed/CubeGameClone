mod chunk;
mod errors;
mod plugin;
mod asset;
mod data;
mod events;
mod state;

use crate::chunk::{ChunkData, PaletteEntry};
use crate::data::block::{AllBlocks, Block, BlockLoader, BlockModel, BlockModelLoader};
use crate::plugin::init::GameInitPlugin;
use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::basic::WHITE;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::wireframe::{NoWireframe, WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::{RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bitvec::field::BitField;
use bitvec::prelude::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::BitViewSized;
use rand::distr::Uniform;
use rand::Rng;
use std::f32::consts::PI;
use crate::asset::BlockMaterial;
use crate::plugin::texture::{BlockTextures, TexturePlugin};
use crate::state::MainGameState;

#[derive(Component)]
struct MainCamera;

#[derive(Debug, Resource)]
struct CameraSettings {
    pub pitch_sensitivity: f32,
    pub yaw_sensitivity: f32,
    pub fov: f32,
    movement_speed: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            pitch_sensitivity: 0.75,
            yaw_sensitivity: 0.75,
            fov: 90.0,
            movement_speed: 5.0
        }
    }
}

#[derive(Resource, Default)]
struct SetupState {
    setup: bool
}


#[derive(Resource)]
struct TestChunk {
    inner: ChunkData
}
impl TestChunk {
    fn new() -> Self {
        let mut palette = vec![
            PaletteEntry::new("stone"),
            PaletteEntry::new("dirt"),
            PaletteEntry::new("oak_planks"),
            // PaletteEntry::new("diamond_ore"),
            // PaletteEntry::new("iron_ore"),
        ];

        // calcualtes the closest power of two id size for the palette.
        let id_size = ((palette.len() + 1) as f32).log2().ceil() as usize;


        let mut vec = BitVec::with_capacity(id_size * 32768);
        let mut rng = rand::rng();
        for i in 0..32768 {
            let scaled_idx = i * id_size;
            // 0-4
            let rand_id = rng.sample(Uniform::new(0, palette.len() + 1).unwrap());

            if rand_id != 0 {
                palette[rand_id - 1].increment_ref_count();
            }
            let arr = rand_id.into_bitarray::<Msb0>();
            // println!("Bitarray: {}", arr);
            
            let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
            // println!("Slice: {}", slice);
            // println!("Generated num: {}", rand_id);

            vec.append(&mut slice.to_bitvec());
        }

        // println!("{:?}", vec);

        
        let mut data = ChunkData::new(vec, palette);
        data.add_palette(PaletteEntry::new("diamond_ore"));
        
        TestChunk {
            inner: data
        }
    }
}


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
            MaterialPlugin::<BlockMaterial>::default(),
            WireframePlugin::default(),
            GameInitPlugin::default(),
            TexturePlugin::default(),
        ))
        .init_asset::<Block>()
        .init_asset::<BlockModel>()
        .init_asset_loader::<BlockLoader>()
        .init_asset_loader::<BlockModelLoader>()
        .init_resource::<CameraSettings>()
        .init_resource::<SetupState>()
        .insert_resource(TestChunk::new())
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .add_systems(Startup, (grab_cursor))
        .add_systems(Update, (handle_input, toggle_wireframe))
        .add_systems(OnEnter(MainGameState::InGame), setup)
        .run();
}


fn handle_input(
    mut transform: Single<&mut Transform, With<MainCamera>>,
    // mut proj: Single<&mut Projection, With<MainCamera>>,
    camera_settings: Res<CameraSettings>,
    timer: Res<Time>,
    kb_input: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
) {
    let delta = mouse_motion.delta;

    let delta_yaw = (camera_settings.yaw_sensitivity * -delta.x).to_radians();
    let delta_pitch = (camera_settings.pitch_sensitivity * -delta.y).to_radians();


    let (yaw_old, pitch_old, roll_old) = transform.rotation.to_euler(EulerRot::YXZ);

    let pitch = (pitch_old + delta_pitch).clamp(
        -89.9 * PI/180.,
        89.9 * PI/180.
    );
    let yaw = yaw_old + delta_yaw;
    let roll = roll_old;
    // important: this is Y X Z, not X Y Z
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll);

    let mut movement = Vec3::ZERO;

    if kb_input.pressed(KeyCode::KeyW) {
        movement += transform.forward().as_vec3();
    }
    if kb_input.pressed(KeyCode::KeyA) {
        movement -= transform.right().as_vec3();
    }
    if kb_input.pressed(KeyCode::KeyS) {
        movement -= transform.forward().as_vec3();
    }
    if kb_input.pressed(KeyCode::KeyD) {
        movement += transform.right().as_vec3();
    }
    // up and down use world up instead - more intuitive
    if kb_input.pressed(KeyCode::Space) {
        movement += vec3(0., 1., 0.);
    }
    if kb_input.pressed(KeyCode::ShiftLeft) {
        movement -= vec3(0., 1., 0.);
    }

    movement = movement.normalize_or_zero();
    transform.translation += movement * camera_settings.movement_speed * timer.delta_secs();
}


fn grab_cursor(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    window.cursor_options.grab_mode = CursorGrabMode::Locked;

    // also hide the cursor
    window.cursor_options.visible = false;
}

// runs once when InGame reached
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    all_blocks: Res<AllBlocks>,
    block_asset: Res<Assets<Block>>,
    block_model_asset: Res<Assets<BlockModel>>,
    block_textures: Res<BlockTextures>,
    camera_settings: Res<CameraSettings>,
    level: Res<TestChunk>,
    mut materials: ResMut<Assets<BlockMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    info!("Loading world...");

    // let texture1: Handle<Image> = asset_server.load("texture/stone.png");
    // let texture2: Handle<Image> = asset_server.load("dirt.png");
    // let texture3: Handle<Image> = asset_server.load("oak_planks.png");

    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: camera_settings.fov.to_radians(),
            ..default()
        }),
        MainCamera,
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(25.0, 50.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y)
    ));

    let array_texture = block_textures.array_texture.clone();

    let mesh = meshes.add(chunk::create_chunk_mesh(&level.inner, &all_blocks, &block_asset, &block_model_asset, &block_textures));
    let material = materials.add(BlockMaterial {
        array_texture
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0., 0., 0.)
    ));
}


fn toggle_wireframe(
    kb_input: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<WireframeConfig>,
    mut to_toggle: Query<&mut Visibility, (With<Mesh3d>, Without<NoWireframe>)>,
) {

    // toggles on and off wireframe
    if kb_input.just_pressed(KeyCode::KeyZ) {
        config.global = !config.global;
        // for mut vis in to_toggle.iter_mut() {
        //     *vis = match config.global {
        //         true => Visibility::Hidden,
        //         false => Visibility::Visible,
        //     }
        // }
    }
}


fn _create_cube() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                // top (facing towards +y)
                [0., 1., 0.],
                [1., 1., 0.],
                [1., 1., 1.],
                [0., 1., 1.],
                // bottom   (-y)
                [0., 0., 0.],
                [1., 0., 0.],
                [1., 0., 1.],
                [0., 0., 1.],
                // right    (+x)
                [1., 0., 0.],
                [1., 0., 1.],
                [1., 1., 1.], // This vertex is at the same position as vertex with index 2, but they'll have different UV and normal
                [1., 1., 0.],
                // left     (-x)
                [0., 0., 0.],
                [0., 0., 1.],
                [0., 1., 1.],
                [0., 1., 0.],
                // back     (+z)
                [0., 0., 1.],
                [0., 1., 1.],
                [1., 1., 1.],
                [1., 0., 1.],
                // forward  (-z)
                [0., 0., 0.],
                [0., 1., 0.],
                [1., 1., 0.],
                [1., 0., 0.],
            ]
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![
                // Assigning the UV coords for the top side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],
                // Assigning the UV coords for the bottom side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],
                // Assigning the UV coords for the right side.
                [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0],
                // Assigning the UV coords for the left side.
                [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0],
                // Assigning the UV coords for the back side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],
                // Assigning the UV coords for the forward side.
                [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0],

            ]
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![
                // Normals for the top side (towards +y)
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            // Normals for the bottom side (towards -y)
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, -1.0, 0.0],
                // Normals for the right side (towards +x)
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                // Normals for the left side (towards -x)
                [-1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                // Normals for the back side (towards +z)
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                // Normals for the forward side (towards -z)
                [0.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
                [0.0, 0.0, -1.0],
            ],
        )
        .with_inserted_indices(Indices::U32(vec![
            0,3,1 , 1,3,2, // triangles making up the top (+y) facing side.
            4,5,7 , 5,6,7, // bottom (-y) 0, 1, 3, 1, 2, 3,
            8,11,9 , 9,11,10, // right (+x) 0, 3, 1, 1, 3, 2,
            12,13,15 , 13,14,15, // left (-x) 0, 1, 3, 1, 2, 3,
            16,19,17 , 17,19,18, // back (+z) 0, 3, 1, 1, 3, 2,
            20,21,23 , 21,22,23, // forward (-z) 0, 1, 3, 1, 2, 3,
        ]))
}