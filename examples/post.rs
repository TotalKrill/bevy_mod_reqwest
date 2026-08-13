use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;
use serde::Serialize;

// example towards the local CORS test server /posts endpoint
#[derive(Serialize)]
struct Post {
    title: String,
    body: String,
    user_id: usize,
}

const BASE_URL: &str = match option_env!("BEVY_MOD_REQWEST_EXAMPLE_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8090",
};

fn send_requests(mut client: BevyReqwest) {
    let url = format!("{BASE_URL}/posts");
    let body = Post {
        title: "hello".into(),
        body: "world".into(),
        user_id: 1,
    };
    let req = client.post(url).json(&body).build().unwrap();
    client
        .send(req)
        .on_response(|req: On<ReqwestResponseEvent>| {
            let req = req.event();
            let res = req.as_str();
            bevy::log::info!("return data: {res:?}");
        });
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin::default())
        .add_plugins(ReqwestPlugin::default())
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(2))),
        )
        .run();
}
