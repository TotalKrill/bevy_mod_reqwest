# bevy_mod_reqwest

This crate helps when trying to use reqwest with bevy, without having to deal with async stuff, and it works on both web and and native
( only tested on x86_64 and wasm for now)


## Example

``` rust
use std::time::Duration;

use bevy::{log::LogPlugin, prelude::*, time::common_conditions::on_timer};
use bevy_mod_reqwest::*;

fn send_requests(mut client: BevyReqwest) {
    let url = "https://www.boredapi.com/api/activity";
    let req = client.get(url).build().unwrap();
    // will run the callback, and remove the created entity after callback
    client.send(
        req,
        On::run(|req: Listener<ReqResponse>| {
            let res = req.as_str();
            bevy::log::info!("return data: {res:?}");
        }),
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
