use std::collections::VecDeque;
use std::time::Duration;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use crate::world::camera::MainCamera;

#[derive(Default)]
pub struct GameUiPlugin;
impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app

            .add_systems(Startup, build_debug_ui)
            .add_systems(Update, (update_fps_text, update_position))
        ;
    }
}

#[derive(Component)]
struct FpsMeter;

#[derive(Component)]
struct Position;

fn build_debug_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>
) {
    let root = commands.spawn(
        Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            justify_content: JustifyContent::FlexStart,
            ..default()
        }
    ).id();

    let left_col = commands.spawn(
        Node {
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexStart,
            // margin: UiRect::axes(Val::Px(15.), Val::Px(5.)),
            ..default()
        }
    ).with_children(|builder| {
        builder.spawn((
            Text::new("Debug Info"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
        ));

        builder.spawn((
            Text::new(" ms/frame"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            FpsMeter,
        ));

        builder.spawn((
            Text::new("x: 0.0, y: 0.0, z: 0.0"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            Position
        ));
    }).id();



    commands.entity(root).add_children(&[left_col]);
}


fn update_fps_text(
    mut fps_history: Local<VecDeque<f64>>,
    mut time_history: Local<VecDeque<Duration>>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    query: Single<Entity, With<FpsMeter>>,
    mut writer: TextUiWriter,
) {
    time_history.push_front(time.elapsed());
    time_history.truncate(120);
    let avg_fps = (time_history.len() as f64)
        / (time_history.front().copied().unwrap_or_default()
        - time_history.back().copied().unwrap_or_default())
        .as_secs_f64()
        .max(0.0001);
    fps_history.push_front(avg_fps);
    fps_history.truncate(120);


    let entity = query.into_inner();

    let mut frame_time = time.delta_secs_f64();
    if let Some(frame_time_diagnostic) =
        diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
    {
        if let Some(frame_time_smoothed) = frame_time_diagnostic.smoothed() {
            frame_time = frame_time_smoothed;
        }
    }

    *writer.text(entity, 0) = format!("{avg_fps:.1} fps | {frame_time:.1} ms");

}

fn update_position(
    camera: Single<&Transform, With<MainCamera>>,
    position: Single<Entity, With<Position>>,
    mut writer: TextUiWriter,
) {
    let pos = camera.translation;
    let (x, y, z) = (pos.x, pos.y, pos.z);
    *writer.text(position.into_inner(), 0) = format!("x: {x:.4}, y: {y:.4}, z: {z:.4}");
}