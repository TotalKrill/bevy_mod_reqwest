use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;

fn send_requests(mut client: BevyReqwest) {
    let url = "https://jsonplaceholder.typicode.com/posts";
    let req = client.get(url).body("some data to send").build().unwrap();
    // sends the request, and ignores any response, see the other examples for other uses
    client.fire_and_forget(req);
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
