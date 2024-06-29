use std::ops::{Deref, DerefMut};

use bevy::ecs::system::SystemParam;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::{log, prelude::*};
use bevy_eventlistener::prelude::*;

pub use bevy_eventlistener;
pub use bevy_eventlistener::prelude::{Listener, On};
pub use reqwest;

#[cfg(target_family = "wasm")]
use crossbeam_channel::{bounded, Receiver};

pub use reqwest::header::HeaderMap;
pub use reqwest::{StatusCode, Version};
use serde::de::DeserializeOwned;

#[cfg(not(target_family = "wasm"))]
use {bevy::tasks::Task, futures_lite::future};

/// Plugin that allows to send http request using the [reqwest](https://crates.io/crates/reqwest) library from
/// inside bevy.
/// The plugin uses [bevy_eventlister](https://crates.io/crates/bevy_eventlistener) to provide callback style events
/// when the http requests finishes.
/// supports both wasm and native
pub struct ReqwestPlugin {
    pub automatically_name_requests: bool,
}
impl Default for ReqwestPlugin {
    fn default() -> Self {
        Self {
            automatically_name_requests: false,
        }
    }
}
impl Plugin for ReqwestPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<ReqwestClient>() {
            app.init_resource::<ReqwestClient>();
        }
        app.add_plugins(EventListenerPlugin::<ReqResponse>::default());
        // app.add_plugins(EventListenerPlugin::<ReqError>::default());
        app.add_systems(PreUpdate, Self::start_handling_requests);
        if self.automatically_name_requests {
            app.add_systems(Update, Self::add_name_to_requests);
        }
        //
        app.add_systems(
            Update,
            (
                // These systems are chained as the callbacks are triggered in PreUpdate
                // So if remove_finished_requests runs after poll_inflight_requests_to_bytes
                // the entity will be removed before the callback is triggered.
                Self::remove_finished_requests,
                Self::poll_inflight_requests_to_bytes,
            )
                .chain(),
        );
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
                let task = {
                    let (tx, task) = bounded(1);
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
                    task
                };

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

    /// despawns finished reqwests if marked to be despawned
    fn remove_finished_requests(
        mut commands: Commands,
        q: Query<
            Entity,
            (
                With<DespawnReqwestEntity>,
                Without<ReqwestInflight>,
                Without<ReqRequest>,
            ),
        >,
    ) {
        for e in q.iter() {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
        }
    }

    fn poll_inflight_requests_to_bytes(
        mut commands: Commands,
        mut requests: Query<(Entity, &mut ReqwestInflight)>,
        mut ew_ok: EventWriter<ReqResponse>,
    ) {
        for (entity, mut request) in requests.iter_mut() {
            bevy::log::debug!("polling: {entity:?}");
            if let Some((result, parts)) = request.poll() {
                match result {
                    Ok(body) => {
                        let parts = parts.unwrap();
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
                if let Some(mut ec) = commands.get_entity(entity) {
                    ec.remove::<ReqwestInflight>();
                }
            }
        }
    }

    /// System that automatically adds a name to http request entites if they are unnamed
    fn add_name_to_requests(
        mut commands: Commands,
        requests_without_name: Query<(Entity, &ReqRequest), (Added<ReqRequest>, Without<Name>)>,
    ) {
        for (entity, request) in requests_without_name.iter() {
            let Some(request) = request.as_ref() else {
                continue;
            };

            let url = request.url().path().to_string();

            commands
                .entity(entity)
                .insert(Name::new(format!("http: {url}")));
        }
    }
}

#[derive(SystemParam)]
/// Systemparam to have a shorthand for creating http calls in systems
pub struct BevyReqwest<'w, 's> {
    commands: Commands<'w, 's>,
    client: Res<'w, ReqwestClient>,
}

impl<'w, 's> BevyReqwest<'w, 's> {
    /// sends the http request as a new entity, that is despawned upon completion
    pub fn send(&mut self, req: reqwest::Request, onresponse: On<ReqResponse>) {
        self.commands
            .spawn((ReqRequest::new(req), onresponse, DespawnReqwestEntity));
    }

    /// sends the http request as a new entity, that is despawned upon completion, and ignore any responses
    pub fn fire_and_forget(&mut self, req: reqwest::Request) {
        self.commands
            .spawn((ReqRequest::new(req), DespawnReqwestEntity));
    }
    /// sends the http request attached to an existing entity, this does not despawn the entity once completed
    pub fn send_using_entity(
        &mut self,
        entity: Entity,
        req: reqwest::Request,
        onresponse: On<ReqResponse>,
    ) {
        let Some(mut ec) = self.commands.get_entity(entity) else {
            log::error!("Failed to create entity");
            return;
        };
        log::info!("inserting request on entity: {:?}", entity);
        ec.insert((ReqRequest::new(req), onresponse));
    }
    /// get access to the underlying ReqwestClient
    pub fn client(&self) -> &reqwest::Client {
        &self.client.0
    }
}

impl<'w, 's> Deref for BevyReqwest<'w, 's> {
    type Target = reqwest::Client;

    fn deref(&self) -> &Self::Target {
        self.client()
    }
}

#[derive(Component)]
/// Marker component that is used to despawn an entity if the reqwest is finshed
pub struct DespawnReqwestEntity;

#[derive(Resource)]
/// Wrapper around the ReqwestClient, that when inserted as a resource will start connection pools towards
/// the hosts, and also allows all the configuration from the ReqwestLibrary such as setting default headers etc
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

#[derive(Component, Deref)]
#[component(storage = "SparseSet")]
/// we have to use an option to be able to ".take()" later on when moving this into a an [InflightRequest]
// that is being polled once per frame
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
/// information about the response used to transfer headers between different stages in the async code
struct Parts {
    /// the `StatusCode`
    pub(crate) status: StatusCode,

    /// the headers of the response
    pub(crate) headers: HeaderMap,
}

#[derive(Clone, Event, EntityEvent)]
/// the resulting data from a finished request is found here
pub struct ReqResponse {
    #[target]
    target: Entity,
    bytes: bytes::Bytes,
    status: StatusCode,
    headers: HeaderMap,
}

#[cfg(feature = "json")]
/// tries to deserialize the response using json into the provided struct, and then overwrite a bevy resource if it succeds
pub fn deserialize_json_into_resource<'de, R>() -> On<ReqResponse>
where
    R: Resource + DeserializeOwned,
{
    let on = On::<ReqResponse>::run(|mut resource: ResMut<R>, req: Listener<ReqResponse>| {
        match req.deserialize_json::<R>() {
            Ok(s) => {
                // do stuff
                *resource = s;
            }
            Err(e) => {
                log::error!("Resource update failed: {e}");
            }
        }
    });
    on
}
#[cfg(feature = "msgpack")]
/// tries to deserialize the response using msgpack into the provided struct, and then overwrite a bevy resource if it succeds
pub fn deserialize_msgpack_into_resource<'de, R>() -> On<ReqResponse>
where
    R: Resource + DeserializeOwned,
{
    let on = On::<ReqResponse>::run(|mut resource: ResMut<R>, req: Listener<ReqResponse>| {
        match req.deserialize_msgpack::<R>() {
            Ok(s) => {
                // do stuff
                *resource = s;
            }
            Err(e) => {
                log::error!("Resource update failed: {e}");
            }
        }
    });
    on
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
    /// retrieve a refernce to the body of the response
    #[inline]
    pub fn body(&self) -> &bytes::Bytes {
        &self.bytes
    }

    /// try to get the body of the response as_str
    pub fn as_str(&self) -> anyhow::Result<&str> {
        let s = std::str::from_utf8(&self.bytes)?;
        Ok(s)
    }
    /// try to get the body of the response as an owned string
    pub fn as_string(&self) -> anyhow::Result<String> {
        Ok(self.as_str()?.to_string())
    }
    #[cfg(feature = "json")]
    /// try to deserialize the body of the response using json
    pub fn deserialize_json<'de, T: serde::Deserialize<'de>>(&'de self) -> anyhow::Result<T> {
        Ok(serde_json::from_str(self.as_str()?)?)
    }

    #[cfg(feature = "msgpack")]
    /// try to deserialize the body of the response using msgpack
    pub fn deserialize_msgpack<'de, T: serde::Deserialize<'de>>(&'de self) -> anyhow::Result<T> {
        Ok(rmp_serde::decode::from_slice(self.body())?)
    }
    #[inline]
    /// Get the `StatusCode` of this `Response`.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[inline]
    /// Get the `Headers` of this `Response`.
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
