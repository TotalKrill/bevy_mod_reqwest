use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Bored {
    pub activity: String,
    pub price: f32,
    pub participants: f32,
}

fn send_requests(mut client: BevyReqwest) {
    let url = "https://bored-api.appbrewery.com/random";

    // use regular reqwest http calls, then poll them to completion.
    let reqwest_request = client.get(url).build().unwrap();

    client
        // Sends the created http request
        .send(reqwest_request)
        // The response from the http request can be reached using an observersystem
        .on_json_response(|trigger: Trigger<JsonResponse<Bored>>| {
            let data: &Bored = &trigger.event().0;
            // let headers = req.response_headers();
            bevy::log::info!("data: {data:?}");
        })
        // In case of request error, it can be reached using an observersystem
        .on_error(|trigger: Trigger<ReqwestErrorEvent>| {
            let e = &trigger.event().0;
            bevy::log::info!("error: {e:?}");
        });
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin::default())
        .add_plugins(ReqwestPlugin::default())
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(5))),
        )
        .run();
}
