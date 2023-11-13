use bevy::{log::LogPlugin, prelude::*};
use bevy_mod_reqwest::*;

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, LogPlugin::default(), ReqwestPlugin))
        .add_systems(Update, (send_requests, handle_responses,))
        .insert_resource(ReqTimer(Timer::new(
            std::time::Duration::from_secs(1),
            TimerMode::Repeating,
        )))
        .run();
}

#[derive(Resource)]
struct ReqTimer(pub Timer);

fn send_requests(mut commands: Commands, time: Res<Time>, mut timer: ResMut<ReqTimer>) {
    timer.0.tick(time.delta());

    if timer.0.just_finished() {
        if let Ok(url) = "https://www.boredapi.com/api/activity".try_into() {
            let req = reqwest::Request::new(reqwest::Method::GET, url);
            let req = ReqwestRequest::new(req);
            commands.spawn(req);
        }
    }
}

fn handle_responses(mut commands: Commands, results: Query<(Entity, &ReqwestBytesResult)>) {
    for (e, res) in results.iter() {
        let string = res.as_str().unwrap();
        bevy::log::info!("{string}");

        // Done with this entity
        commands.entity(e).despawn_recursive();
    }
}
