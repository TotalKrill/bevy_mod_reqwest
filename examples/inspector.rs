// use bevy::prelude::*;
// use bevy::time::common_conditions::on_timer;
// use bevy_inspector_egui::quick::WorldInspectorPlugin;
// use bevy_mod_reqwest::*;
// use std::time::Duration;

// fn main() {
//     App::new()
//         .add_plugins((
//             DefaultPlugins,
//             WorldInspectorPlugin::default(),
//             ReqwestPlugin {
//                 automatically_name_requests: true,
//             },
//         ))
//         .add_systems(
//             Update,
//             send_ignored_request.run_if(on_timer(Duration::from_secs(1))),
//         )
//         .add_systems(
//             Update,
//             send_requests_that_remain.run_if(on_timer(Duration::from_secs(1))),
//         )
//         .add_systems(
//             Update,
//             (spawn_requests_with_generated_name.run_if(on_timer(Duration::from_secs(3))),),
//         )
//         .run();
// }

// fn send_ignored_request(mut client: BevyReqwest) {
//     let url = "https://www.boredapi.com/api";
//     let req = client.get(url).build().unwrap();
//     // ignores any responses and removes the created entity
//     client.fire_and_forget(req);
// }

// fn spawn_requests_with_generated_name(mut client: BevyReqwest) {
//     let url = "https://www.boredapi.com/api";
//     let req = client.get(url).build().unwrap();
//     // will run the callback, and remove the created entity after callback
//     client.send(
//         req,
//         On::run(|req: Listener<ReqResponse>| {
//             let res = req.as_str();
//             bevy::log::info!("return data: {res:?}");
//         }),
//     );
// }

// #[derive(Component)]
// pub struct Data {
//     pub s: String,
// }

// fn send_requests_that_remain(mut commands: Commands, mut client: BevyReqwest) {
//     let url = "https://www.boredapi.com/api/activity";
//     let req = client.get(url).build().unwrap();
//     let e = commands
//         .spawn(Name::new("a http request to bored api"))
//         .id();
//     // this will not automatically remove the entity after return of data, wich will leave a bunch of entities visible in the inspector
//     client.send_using_entity(
//         e,
//         req,
//         On::target_commands_mut(|ev, tc| {
//             let req: &ReqResponse = &ev;
//             let res: String = req.as_string().unwrap();
//             bevy::log::info!("return data: {res:?}");
//             tc.insert(Data { s: res.into() });
//         }),
//     );
// }
