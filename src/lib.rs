use std::ops::DerefMut;

use bevy::tasks::AsyncComputeTaskPool;
use bevy::{log, prelude::*};
pub use reqwest;

#[cfg(target_family = "wasm")]
use crossbeam_channel::{bounded, Receiver};

#[cfg(not(target_family = "wasm"))]
use {bevy::tasks::Task, futures_lite::future};

#[derive(Resource)]
pub struct ReqwestClient(pub reqwest::Client);
impl Default for ReqwestClient {
    fn default() -> Self {
        Self(reqwest::Client::new())
    }
}

impl std::ops::Deref for ReqwestClient {
    type Target = reqwest::Client;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for ReqwestClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// we have to use an option to be able to ".take()" later
#[derive(Component, Deref)]
pub struct ReqwestRequest(pub Option<reqwest::Request>);

impl ReqwestRequest {
    pub fn new(request: reqwest::Request) -> Self {
        Self(Some(request))
    }
}

impl Into<ReqwestRequest> for reqwest::Request {
    fn into(self) -> ReqwestRequest {
        ReqwestRequest(Some(self))
    }
}

#[cfg(target_family = "wasm")]
#[derive(Component, Deref)]
pub struct ReqwestInflight(Receiver<reqwest::Result<bytes::Bytes>>);

/// Dont touch these, its just to poll once every request
#[cfg(not(target_family = "wasm"))]
#[derive(Component, Deref)]
pub struct ReqwestInflight(Task<reqwest::Result<bytes::Bytes>>);

#[derive(Component, Deref, Debug)]
pub struct ReqwestBytesResult(pub reqwest::Result<bytes::Bytes>);

impl ReqwestBytesResult {
    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            Ok(string) => Some(std::str::from_utf8(string).ok()?),
            Err(_) => None,
        }
    }
    pub fn as_string(&mut self) -> Option<String> {
        Some(self.as_str()?.into())
    }

    #[cfg(feature = "msgpack")]
    pub fn decode_msgpack<'de, T: serde::Deserialize<'de>>(&'de self) -> Option<T> {
        if let Ok(val) = &self.0 {
            match rmp_serde::decode::from_slice(val) {
                Ok(json) => Some(json),
                Err(e) => {
                    log::error!("failed to deserialize: {e:?}");
                    None
                }
            }
        } else {
            None
        }
    }
    pub fn deserialize_json<'de, T: serde::Deserialize<'de>>(&'de self) -> Option<T> {
        match serde_json::from_str(self.as_str()?) {
            Ok(json) => Some(json),
            Err(e) => {
                log::error!("failed to deserialize: {e:?}");
                None
            }
        }
    }
}

pub struct ReqwestPlugin;
impl Plugin for ReqwestPlugin {
    fn build(&self, app: &mut App) {
        if !app.world.contains_resource::<ReqwestClient>() {
            app.init_resource::<ReqwestClient>();
        }
        app.add_systems(Update, Self::start_handling_requests);
        app.add_systems(Update, Self::poll_inflight_requests_to_bytes);
    }
}

//TODO: Make type generic, and we can create systems for JSON and TEXT requests
impl ReqwestPlugin {
    fn start_handling_requests(
        mut commands: Commands,
        http_client: ResMut<ReqwestClient>,
        mut requests: Query<(Entity, &mut ReqwestRequest), Added<ReqwestRequest>>,
    ) {
        let thread_pool = AsyncComputeTaskPool::get();
        for (entity, mut request) in requests.iter_mut() {
            bevy::log::debug!("Creating: {entity:?}");
            // if we take the data, we can use it
            if let Some(request) = request.0.take() {
                let client = http_client.0.clone();

                // wasm implementation
                #[cfg(target_family = "wasm")]
                let (tx, task) = bounded(1);
                #[cfg(target_family = "wasm")]
                thread_pool
                    .spawn(async move {
                        let r = client.execute(request).await;
                        let r = match r {
                            Ok(res) => res.bytes().await,
                            Err(r) => Err(r),
                        };
                        tx.send(r).ok();
                    })
                    .detach();

                // otherwise
                #[cfg(not(target_family = "wasm"))]
                let task = {
                    thread_pool.spawn(async move {
                        #[cfg(not(target_family = "wasm"))]
                        let r = async_compat::Compat::new(async {
                            client.execute(request).await?.bytes().await
                        })
                        .await;
                        r
                    })
                };
                // put it as a component to be polled, and remove the request, it has been handled
                commands.entity(entity).insert(ReqwestInflight(task));
                commands.entity(entity).remove::<ReqwestRequest>();
            }
        }
    }

    fn poll_inflight_requests_to_bytes(
        mut commands: Commands,
        // Very important to have the Without, otherwise we get task failure upon completed task
        mut requests: Query<(Entity, &mut ReqwestInflight), Without<ReqwestBytesResult>>,
    ) {
        for (entity, mut request) in requests.iter_mut() {
            bevy::log::debug!("polling: {entity:?}");

            #[cfg(target_family = "wasm")]
            if let Ok(result) = request.0.try_recv() {
                // move the result over to a new component
                commands
                    .entity(entity)
                    .insert(ReqwestBytesResult(result))
                    .remove::<ReqwestInflight>();
            }

            #[cfg(not(target_family = "wasm"))]
            if let Some(result) = future::block_on(future::poll_once(&mut request.0)) {
                // move the result over to a new component
                commands
                    .entity(entity)
                    .insert(ReqwestBytesResult(result))
                    .remove::<ReqwestInflight>();
            }
        }
    }
}
