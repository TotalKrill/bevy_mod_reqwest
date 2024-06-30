# bevy_mod_reqwest

[![crates.io](https://img.shields.io/crates/v/bevy_mod_reqwest)](https://crates.io/crates/bevy_mod_reqwest)
[![docs.rs](https://docs.rs/bevy_mod_reqwest/badge.svg)](https://docs.rs/bevy_mod_reqwest)

This crate helps when trying to use reqwest with bevy, without having to deal with async stuff, and it works on both web and and native
( only tested on x86_64 and wasm for now)


## Example

``` rust
use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;

fn send_requests(mut client: BevyReqwest) {
    let url = "https://bored-api.appbrewery.com/random";

    // use regular reqwest http calls, then poll them to completion.
    let reqwest_request = client.get(url).build().unwrap();
    // will run the callback, and remove the created entity after callback
    client.send(
        reqwest_request,
        // When the http request has finished, the following system will be run
        |trigger: Trigger<ReqwestResponseEvent>| {
            let response = trigger.event();
            let data = response.as_str();
            let status = response.status();

            // let headers = req.response_headers();
            bevy::log::info!("code: {status}, data: {data:?}");
        },
    );
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
```
