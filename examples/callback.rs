use bevy_eventlistener::callbacks::ListenerInput;
// relevant systems and include
use bevy_mod_reqwest::*;
// rest of example to make it run
use bevy::{
    log::{self},
    prelude::*,
    time::common_conditions::on_timer,
};
use reqwest::Url;
use std::time::Duration;

/// implement the [Event] to able to use this from an eventreader, dont forget to add the event to the app
#[derive(serde::Deserialize, Debug, Event)]
pub struct Bored {
    pub activity: String,
    pub r#type: String,
    pub participants: f32,
    pub accessibility: f32,
    pub price: f32,
    pub link: String,
}

/// this is one way to automatically turn the responses into events, which is
/// the prefered way, since it allows parallelism according to
/// [example](https://github.com/aevyrie/bevy_eventlistener/blob/main/examples/event_listeners.rs)
impl From<ListenerInput<ReqResponse>> for Bored {
    fn from(value: ListenerInput<ReqResponse>) -> Self {
        let s = value.deserialize_json().unwrap();
        s
    }
}
/// builds the http requests
fn send_requests(mut bevyreq: BevyReqwest) {
    log::info!("Sending activity request");
    let url: Url = "https://www.boredapi.com/api/activity".try_into().unwrap();
    let reqwest = bevyreq.client().get(url).build().unwrap();
    bevyreq.send(
        // the http request
        reqwest,
        // what to do when the api call is complete
        On::send_event::<Bored>(),
    );
}

/// here you can do anything with the data from the events
fn handle_events(mut events: EventReader<Bored>) {
    for ev in events.read() {
        log::info!("got respoonse: {ev:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(ReqwestPlugin::default())
        .add_event::<Bored>()
        .add_plugins(bevy::log::LogPlugin::default())
        .add_systems(Update, handle_events)
        .add_systems(
            Update,
            send_requests.run_if(on_timer(Duration::from_secs(4))),
        )
        .run();
}
