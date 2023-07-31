use bevy::{log::LogPlugin, prelude::*};
use bevy_eventlistener::{
    callbacks::{Listener, ListenerMut},
    event_listener::EntityEvent,
    prelude::*,
};
use bevy_mod_reqwest::*;

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(ReqwestPlugin)
        // events that triggers on responses
        .add_systems(Update, send_requests)
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
            commands.spawn((
                ReqwestRequest::new(reqwest::Request::new(reqwest::Method::GET, url)),
                On::<ReqResponse>::run(
                    |mut commands: Commands, req: Listener<ReqResponse>, q: Query<Entity>| {
                        if let Ok(e) = q.get(req.listener()) {
                            // we got resp
                            let st = req.as_str();
                            bevy::log::info!("{st:?}");
                            commands.entity(e).despawn_recursive();
                        }
                    },
                ),
            ));
        }
    }
}
