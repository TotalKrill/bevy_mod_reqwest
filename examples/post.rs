use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;
use serde::Serialize;

// example towards jsonplaceholder.typicod.com/posts
#[derive(Serialize)]
struct Post {
    title: String,
    body: String,
    user_id: usize,
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin::default())
        .add_plugins(ReqwestPlugin)
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(2))),
        )
        .add_systems(Update, handle_responses)
        .run();
}

fn send_requests(mut commands: Commands, reqwest: Res<ReqwestClient>) {
    let url = "https://jsonplaceholder.typicode.com/posts";
    let body = Post {
        title: "hello".into(),
        body: "world".into(),
        user_id: 1,
    };
    let req = reqwest.0.post(url).json(&body).build().unwrap();
    let req = ReqwestRequest::new(req);
    commands.spawn(req);
}

fn handle_responses(mut commands: Commands, results: Query<(Entity, &ReqwestBytesResult)>) {
    for (e, res) in results.iter() {
        let string = res.as_str().unwrap();
        bevy::log::info!("{string}");

        // Done with this entity
        commands.entity(e).despawn_recursive();
    }
}
