use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;

fn send_requests(mut client: BevyReqwest) {
    let url = "https://bored-api.appbrewery.com/random";

    // use regular reqwest http calls, then poll them to completion.
    let reqwest_request = client.get(url).build().unwrap();

    client
        // Sends the created http request
        .send(reqwest_request)
        // The response from the http request can be reached using an observersystem
        .on_response(|trigger: Trigger<ReqwestResponseEvent>| {
            let response = trigger.event();
            let data = response.as_str();
            let status = response.status();
            // let headers = req.response_headers();
            bevy::log::info!("code: {status}, data: {data:?}");
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
