use std::f32::consts::PI;
use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::basic::WHITE;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::wireframe::{NoWireframe, Wireframe, WireframeColor, WireframeConfig, WireframePlugin};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::RenderPlugin;
use bevy::render::settings::{RenderCreation, WgpuSettings};
use bevy::window::{CursorGrabMode, PrimaryWindow};

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

#[derive(Debug, Resource)]
struct BasicLevel {
    data: [u64; 8]
}

struct SingleChunk;

enum Facing {
    North, // +z
    South, // -z
    East, // +x
    West, // -x
    Up, // +y
    Down, // -y
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
            WireframePlugin::default(),
        ))
        .init_resource::<CameraSettings>()
        .insert_resource(BasicLevel {
            data: [
                0b11111111_10001101_00000000_11111111_01011010_01110101_11011101_00001111,
                0b00001101_10111001_00001100_00000000_00000000_00000000_00010000_00000011,
                0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000,
                0b00000000_00000000_00101000_00000000_00000000_00000000_00000000_00000000,
                0b11111111_11111111_11111101_11111111_11110111_11111111_11111111_11111111,
                0b11111111_11111001_11100001_11111111_11100011_11111111_11111111_11111111,
                0b11111111_11111111_11110111_11111111_11110111_11111111_11111111_11111111,
                0b00000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000,
            ]
        })
        .insert_resource(WireframeConfig {
            // The global wireframe config enables drawing of wireframes on every mesh,
            // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
            // regardless of the global configuration.
            global: false,
            // Controls the default color of all wireframes. Used as the default color for global wireframes.
            // Can be changed per mesh using the `WireframeColor` component.
            default_color: WHITE.into(),
        })
        .add_systems(Startup, (setup, grab_cursor))
        .add_systems(Update, (handle_input, toggle_wireframe))
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
    if kb_input.pressed(KeyCode::Space) {
        movement += transform.up().as_vec3();
    }
    if kb_input.pressed(KeyCode::ShiftLeft) {
        movement -= transform.up().as_vec3();
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

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    camera_settings: Res<CameraSettings>,
    level: Res<BasicLevel>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {






    let texture: Handle<Image> = asset_server.load("stone.png");

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

    let mesh = meshes.add(create_chunk_mesh(&level));
    let material = materials.add(texture.clone());

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

fn create_chunk_mesh(level: &Res<BasicLevel>) -> Mesh {
    // faces to make a mesh for
    let mut faces = Vec::<(Facing, Vec3)>::new();

    let mut positions = Vec::<[f32; 3]>::new();
    let mut uv0s = Vec::<[f32; 2]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();


    for y in 0..8 {
        let mut working_data = level.data[y];
        let mut i = working_data.trailing_zeros() as usize;
        while i < 64 {
            let (x, z) = (i / 8, i % 8);
            if should_make_face(Facing::North, level.data, (i, y)) {
                faces.push((Facing::North, vec3(x as f32, y as f32, z as f32)));
            }
            if should_make_face(Facing::South, level.data, (i, y)) {
                faces.push((Facing::South, vec3(x as f32, y as f32, z as f32)));
            }
            if should_make_face(Facing::East, level.data, (i, y)) {
                faces.push((Facing::East, vec3(x as f32, y as f32, z as f32)));
            }
            if should_make_face(Facing::West, level.data, (i, y)) {
                faces.push((Facing::West, vec3(x as f32, y as f32, z as f32)));
            }
            if should_make_face(Facing::Up, level.data, (i, y)) {
                faces.push((Facing::Up, vec3(x as f32, y as f32, z as f32)));
            }
            if should_make_face(Facing::Down, level.data, (i, y)) {
                faces.push((Facing::Down, vec3(x as f32, y as f32, z as f32)));
            }

            // zero out nth bit and move on
            let mask: u64 = !(1 << i);
            working_data = working_data & mask;
            i = working_data.trailing_zeros() as usize;
        }
    }

    let mut index_offset = 0;
    for (dir, pos_offset) in faces {

        // face data
        let (face_pos, face_uv0, face_normal, face_index) = face_data(dir);

        // offsets and adds vertices
        for j in 0..4 {
            let (pos, uv0, normal) = (face_pos[j], face_uv0[j], face_normal[j]);
            // add offset for pos
            let new_pos = [pos[0] + pos_offset.x, pos[1] + pos_offset.y, pos[2] + pos_offset.z];
            positions.push(new_pos);
            uv0s.push(uv0);
            normals.push(normal);
        }
        // add index offset for indices
        for j in 0..6 {
            indices.push(face_index[j] + index_offset);
        }

        index_offset += 4;
    }


    // creates the chunk mesh
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0s)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}


// outputs vertex specific data for this block and face
fn face_data(facing: Facing) -> ([[f32; 3]; 4], [[f32; 2]; 4], [[f32; 3]; 4], [u32; 6]) {
    match facing {
        Facing::North => (
            [ [0., 0., 1.], [0., 1., 1.], [1., 1., 1.], [1., 0., 1.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], ],
            [ [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], ],
            [ 0,3,1, 1,3,2, ],
            ),
        Facing::South => (
            [ [0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], ],
            [ [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], ],
            [ 0,1,3, 1,2,3, ],
            ),
        Facing::East => (
            [ [1., 0., 0.], [1., 0., 1.], [1., 1., 1.], [1., 1., 0.], ],
            [ [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], ],
            [ [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], ],
            [ 0,3,1, 1,3,2, ],
            ),
        Facing::West => (
            [ [0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.], ],
            [ [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], ],
            [ [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], ],
            [ 0,1,3, 1,2,3 ],
            ),
        Facing::Up => (
            [ [0., 1., 0.], [1., 1., 0.], [1., 1., 1.], [0., 1., 1.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0] ],
            [ [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], ],
            [ 0,3,1, 1,3,2 ],
        ),
        Facing::Down => (
            [ [0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], ],
            [ [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], ],
            [ 0,1,3, 1,2,3 ]
        ),
    }
}

fn should_make_face(facing: Facing, data: [u64; 8], pos: (usize, usize)) -> bool {

    let (i, y) = pos;

    // temporary: we wont have to do this for actual chunks since you can just check the next chunk over
    let (x, z) = (i / 8, i % 8);
    match facing {
        Facing::North => {
            if(z == 7) { return true; };
        }
        Facing::South => {
            if(z == 0) { return true; };
        }
        Facing::East => {
            if(x == 7) { return true; };
        }
        Facing::West => {
            if(x == 0) { return true; };
        }
        Facing::Up => {
            if(y == 7) { return true; };
        }
        Facing::Down => {
            if(y == 0) { return true; };
        }
    };

    let (new_pos, new_y) = new_block(facing, i, y);
    let val = data[new_y] & (1 << new_pos);
    val == 0
}

// no guarantee these are in bounds
fn new_block(facing: Facing, index: usize, y: usize) -> (usize, usize) {
    match facing {
        Facing::North => (index + 1, y),
        Facing::South => (index - 1, y),
        Facing::East =>  (index + 8, y),
        Facing::West =>  (index - 8, y),
        Facing::Up =>    (index, y + 1),
        Facing::Down =>  (index, y - 1),
    }
}



fn create_cube() -> Mesh {
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