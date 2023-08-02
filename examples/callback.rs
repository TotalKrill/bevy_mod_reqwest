// relevant systems and include
use bevy_mod_reqwest::*;

fn send_requests(mut commands: Commands) {
    let url = "https://www.boredapi.com/api/activity".try_into().unwrap();

    commands.spawn((
        ReqRequest::new(reqwest::Request::new(reqwest::Method::GET, url)),
        On::<ReqResponse>::run(
            |mut commands: Commands, req: Listener<ReqResponse>, q: Query<Entity>| {
                // handle the response on this entity, using the Listener
                // from bevy_eventlistener
                if let Ok(e) = q.get(req.listener()) {
                    if let Some(bored) = req.deserialize_json::<Bored>() {
                        info!("Activity: {}", bored.activity);
                    }
                    // Since we just respawn new entites, this entity will
                    // not be reused, so despawn it
                    commands.entity(e).despawn_recursive();
                }
            },
        ),
    ));
}

// rest of example to make it run
use bevy::{log::info, prelude::*, time::common_conditions::on_timer};
use std::time::Duration;

#[derive(serde::Deserialize, Debug)]
pub struct Bored {
    pub activity: String,
    pub r#type: String,
    pub participants: f32,
    pub accessibility: f32,
    pub price: f32,
    pub link: String,
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(ReqwestPlugin)
        .add_plugins(bevy::log::LogPlugin::default())
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(4))),
        )
        .run();
}
