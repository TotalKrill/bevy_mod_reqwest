use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_mod_reqwest::*;
use std::time::Duration;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            WorldInspectorPlugin::default(),
            ReqwestPlugin,
        ))
        .add_systems(Startup, send_ignored_request)
        .add_systems(
            Update,
            (
                send_requests.run_if(on_timer(Duration::from_secs(2))),
                handle_responses.run_if(on_timer(Duration::from_secs(1))),
            ),
        )
        .run();
}

#[derive(Component)]
struct Ignore;

fn send_ignored_request(mut commands: Commands) {
    let Ok(url) = "https://www.boredapi.com/api".try_into() else {
        return;
    };

    let req = reqwest::Request::new(reqwest::Method::GET, url);
    let req = ReqwestRequest::new(req);
    commands.spawn((req, Ignore));
}

fn send_requests(mut commands: Commands) {
    let Ok(url) = "https://www.boredapi.com/api".try_into() else {
        return;
    };

    let req = reqwest::Request::new(reqwest::Method::GET, url);
    let req = ReqwestRequest::new(req);
    commands.spawn(req);
}

fn handle_responses(
    mut commands: Commands,
    results: Query<Entity, (Without<Ignore>, With<ReqwestBytesResult>)>,
) {
    for e in results.iter() {
        commands.entity(e).despawn_recursive();
    }
}
