use std::ops::DerefMut;

use bevy::tasks::AsyncComputeTaskPool;
use bevy::{log, prelude::*};
use bevy_eventlistener::prelude::*;

pub use bevy_eventlistener::prelude::{Listener, On};
pub use reqwest;

#[cfg(target_family = "wasm")]
use crossbeam_channel::{bounded, Receiver};

pub use reqwest::header::HeaderMap;
pub use reqwest::{StatusCode, Version};

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
#[component(storage = "SparseSet")]
pub struct ReqRequest(pub Option<reqwest::Request>);

impl ReqRequest {
    pub fn new(request: reqwest::Request) -> Self {
        Self(Some(request))
    }
}

impl Into<ReqRequest> for reqwest::Request {
    fn into(self) -> ReqRequest {
        ReqRequest(Some(self))
    }
}

type Resp = (reqwest::Result<bytes::Bytes>, Option<Parts>);

/// Dont touch these, its just to poll once every request
#[derive(Component)]
#[component(storage = "SparseSet")]
struct ReqwestInflight {
    #[cfg(not(target_family = "wasm"))]
    res: Task<Resp>,

    #[cfg(target_family = "wasm")]
    res: Receiver<Resp>,
}

impl ReqwestInflight {
    fn poll(&mut self) -> Option<Resp> {
        #[cfg(target_family = "wasm")]
        if let Ok(v) = self.res.try_recv() {
            Some(v)
        } else {
            None
        }

        #[cfg(not(target_family = "wasm"))]
        if let Some(v) = future::block_on(future::poll_once(&mut self.res)) {
            Some(v)
        } else {
            None
        }
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn new(res: Receiver<Resp>) -> Self {
        Self { res }
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn new(res: Task<Resp>) -> Self {
        Self { res }
    }
}

#[derive(Component, Debug)]
/// information about the response
struct Parts {
    /// the `StatusCode`
    pub(crate) status: StatusCode,

    /// the headers of the response
    pub(crate) headers: HeaderMap,
}

#[derive(Clone, Event, EntityEvent)]
pub struct ReqResponse {
    #[target]
    target: Entity,
    bytes: bytes::Bytes,
    status: StatusCode,
    headers: HeaderMap,
}

// #[derive(Clone, Event, EntityEvent)]
// pub struct ReqError {
//     #[target]
//     target: Entity,
// }
// impl ReqError {
//     fn new(target: Entity) -> ReqError {
//         Self { target }
//     }
// }

impl ReqResponse {
    pub fn body(&self) -> &bytes::Bytes {
        &self.bytes
    }

    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }
    pub fn as_string(&self) -> Option<String> {
        Some(self.as_str()?.into())
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
    /// Get the `StatusCode` of this `Response`.
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// Get the `Headers` of this `Response`.
    #[inline]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}

impl ReqResponse {
    pub(crate) fn new(
        target: Entity,
        bytes: bytes::Bytes,
        status: StatusCode,
        headers: HeaderMap,
    ) -> Self {
        Self {
            target,
            bytes,
            status,
            headers,
        }
    }
}

impl ReqwestBytesResult {
    pub fn body(&self) -> &reqwest::Result<bytes::Bytes> {
        &self.body
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.body {
            Ok(string) => Some(std::str::from_utf8(&string).ok()?),
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

    /// Get the `StatusCode` of this `Response`.
    #[inline]
    pub fn status(&self) -> Option<StatusCode> {
        match &self.parts {
            Some(parts) => Some(parts.status),
            None => None,
        }
    }

    /// Get the `Headers` of this `Response`.
    #[inline]
    pub fn headers(&self) -> Option<&HeaderMap> {
        match &self.parts {
            Some(parts) => Some(&parts.headers),
            None => None,
        }
    }
}

pub struct ReqwestPlugin;
impl Plugin for ReqwestPlugin {
    fn build(&self, app: &mut App) {
        if !app.world.contains_resource::<ReqwestClient>() {
            app.init_resource::<ReqwestClient>();
        }
        app.add_plugins(EventListenerPlugin::<ReqResponse>::default());
        // app.add_plugins(EventListenerPlugin::<ReqError>::default());
        app.add_systems(Update, Self::start_handling_requests);
        app.add_systems(Update, Self::poll_inflight_requests_to_bytes);
    }
}

//TODO: Make type generic, and we can create systems for JSON and TEXT requests
impl ReqwestPlugin {
    fn start_handling_requests(
        mut commands: Commands,
        http_client: ResMut<ReqwestClient>,
        mut requests: Query<(Entity, &mut ReqRequest), Added<ReqRequest>>,
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
                            Ok(res) => {
                                let parts = Parts {
                                    status: res.status(),
                                    headers: res.headers().clone(),
                                };
                                (res.bytes().await, Some(parts))
                            }
                            Err(r) => (Err(r), None),
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
                            let p = client.execute(request).await;
                            match p {
                                Ok(res) => {
                                    let parts = Parts {
                                        status: res.status(),
                                        headers: res.headers().clone(),
                                    };
                                    (res.bytes().await, Some(parts))
                                }
                                Err(e) => (Err(e), None),
                            }
                        })
                        .await;
                        r
                    })
                };
                // put it as a component to be polled, and remove the request, it has been handled
                commands.entity(entity).insert(ReqwestInflight::new(task));
                commands.entity(entity).remove::<ReqRequest>();
            }
        }
    }

    fn poll_inflight_requests_to_bytes(
        mut commands: Commands,
        // Very important to have the Without, otherwise we get task failure upon completed task
        mut requests: Query<(Entity, &mut ReqwestInflight)>,
        mut ew_ok: EventWriter<ReqResponse>,
    ) {
        for (entity, mut request) in requests.iter_mut() {
            bevy::log::debug!("polling: {entity:?}");
            if let Some((result, parts)) = request.poll() {
                let parts = parts.unwrap();
                match result {
                    Ok(body) => {
                        // if the response is ok, the other values are already gotten, its safe to unwrap
                        ew_ok.send(ReqResponse::new(
                            entity.clone(),
                            body.clone(),
                            parts.status,
                            parts.headers,
                        ));
                    }
                    Err(err) => {
                        bevy::log::error!("{err:?}");
                        //TODO: figure out a way to include error information in a good way and what are errors
                        // ew_err.send(ReqError::new(e.clone()));
                    }
                }
            }
        }
    }
    fn add_name_to_requests(
        mut commands: Commands,
        requests_without_name: Query<(Entity, &ReqRequest), (Added<ReqRequest>, Without<Name>)>,
    ) {
        for (entity, request) in requests_without_name.iter() {
            let Some(request) = request.as_ref() else {
                continue;
            };

            let url = request.url().path().to_string();

            commands.entity(entity).insert(Name::new(url));
        }
    }
    fn generate_events(
        mut commands: Commands,
        mut ew_ok: EventWriter<ReqResponse>,
        // mut ew_err: EventWriter<ReqError>,
        results: Query<(Entity, &ReqwestBytesResult)>,
    ) {
        for (e, res) in results.iter() {
            match res.body() {
                Ok(body) => {
                    // if the response is ok, the other values are already gotten, its safe to unwrap
                    ew_ok.send(ReqResponse::new(
                        e.clone(),
                        body.clone(),
                        res.status().unwrap(),
                        res.headers().unwrap().clone(),
                    ));
                }
                Err(err) => {
                    bevy::log::error!("{err:?}");
                    //TODO: figure out a way to include error information in a good way and what are errors
                    // ew_err.send(ReqError::new(e.clone()));
                }
            }
            if let Some(mut ec) = commands.get_entity(e) {
                ec.remove::<ReqwestBytesResult>();
            }
        }
    }
}
