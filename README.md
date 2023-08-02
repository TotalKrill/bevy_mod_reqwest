# bevy_mod_reqwest

This crate helps when trying to use reqwest with bevy, without having to deal with async stuff, and it works on both web and and native
( only tested on x86_64 and wasm for now)


## Example

``` rust
use std::time::Duration;

use bevy::{
    log::{info, LogPlugin},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_eventlistener::prelude::*;
use bevy_mod_reqwest::*;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
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
        .add_plugins(LogPlugin::default())
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(4))),
        )
        .run();
}

fn send_requests(mut commands: Commands) {
    let url = "https://www.boredapi.com/api/activity".try_into().unwrap();
    // let url = "https://www.thisaddressdoesnotexist.com"
    //     .try_into()
    //     .unwrap();

    commands.spawn((
        ReqRequest::new(reqwest::Request::new(reqwest::Method::GET, url)),
        On::<ReqResponse>::run(
            |mut commands: Commands, req: Listener<ReqResponse>, q: Query<Entity>| {
                if let Ok(e) = q.get(req.listener()) {
                    commands.entity(e).despawn_recursive();
                    // we got resp
                    if let Some(bored) = req.deserialize_json::<Bored>() {
                        info!("Activity: {}", bored.activity);
                    }
                }
            },
        ),
    ));
}
```
