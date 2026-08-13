use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;

#[derive(Default, Resource)]
// just a vector that stores all the responses as strings to showcase that the `on_response` methods
// are just regular observersystems, that function very much like regular systems
struct History {
    pub responses: Vec<String>,
}

const BASE_URL: &str = match option_env!("BEVY_MOD_REQWEST_EXAMPLE_BASE_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:8090",
};

fn send_requests(mut client: BevyReqwest) {
    let url = format!("{BASE_URL}/random");

    // use regular reqwest http calls, then poll them to completion.
    let reqwest_request = client.get(url).build().unwrap();

    client
        // Sends the created http request
        .send(reqwest_request)
        // The response from the http request can be reached using an observersystem,
        // where the only requirement is that the first parameter in the system is the specific Trigger type
        // the rest is the same as a regular system
        .on_response(
            |trigger: On<ReqwestResponseEvent>, mut history: ResMut<History>| {
                let response = trigger.event();
                let data = response.as_str();
                let status = response.status();
                // let headers = req.response_headers();
                bevy::log::info!("code: {status}, data: {data:?}");
                if let Ok(data) = data {
                    history.responses.push(format!("OK: {data}"));
                }
            },
        )
        // In case of request error, it can be reached using an observersystem as well
        .on_error(
            |trigger: On<ReqwestErrorEvent>, mut history: ResMut<History>| {
                let e = &trigger.event().entity;
                bevy::log::info!("error: {e:?}");
                history.responses.push(format!("ERROR: {e:?}"));
            },
        );
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(LogPlugin::default())
        .add_plugins(ReqwestPlugin::default())
        .init_resource::<History>()
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(5))),
        )
        .run();
}
