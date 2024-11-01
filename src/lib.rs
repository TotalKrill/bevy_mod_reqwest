use std::ops::{Deref, DerefMut};

use bevy::{
    ecs::system::{EntityCommands, IntoObserverSystem, SystemParam},
    prelude::*,
    tasks::{AsyncComputeTaskPool, IoTaskPool},
};

pub use reqwest;

#[cfg(target_family = "wasm")]
use crossbeam_channel::{bounded, Receiver};

#[cfg(feature = "json")]
pub use json::*;

pub use reqwest::header::HeaderMap;
pub use reqwest::{StatusCode, Version};

#[cfg(not(target_family = "wasm"))]
use {bevy::tasks::Task, futures_lite::future};

/// The [`SystemSet`] that Reqwest systems are added to.
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ReqwestSet;

/// Plugin that allows to send http request using the [reqwest](https://crates.io/crates/reqwest) library from
/// inside bevy.
///
/// The plugin uses [`Observer`] systems to provide callbacks when the http requests finishes.
///
/// Supports both wasm and native.
pub struct ReqwestPlugin {
    /// this enables the plugin to insert a new [`Name`] component onto the entity used to drive
    /// the http request to completion, if no Name component already exists
    pub automatically_name_requests: bool,
}
impl Default for ReqwestPlugin {
    fn default() -> Self {
        Self {
            automatically_name_requests: true,
        }
    }
}
impl Plugin for ReqwestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReqwestClient>();

        if self.automatically_name_requests {
            // register a hook on the component to add a name to the entity if it doesnt have one already
            app.world_mut()
                .register_component_hooks::<ReqwestInflight>()
                .on_insert(|mut world, entity, _component_id| {
                    let url = world.get::<ReqwestInflight>(entity).unwrap().url.clone();

                    if let None = world.get::<Name>(entity) {
                        let mut commands = world.commands();
                        let mut entity = commands.get_entity(entity).unwrap();
                        entity.insert(Name::new(format!("http: {url}")));
                    }
                });
        }
        //
        app.add_systems(
            PreUpdate,
            (
                // These systems are chained as the callbacks are triggered in PreUpdate
                // So if remove_finished_requests runs after poll_inflight_requests_to_bytes
                // the entity will be removed before the callback is triggered.
                Self::remove_finished_requests,
                Self::poll_inflight_requests_to_bytes,
            )
                .chain()
                .in_set(ReqwestSet),
        );
    }
}

//TODO: Make type generic, and we can create systems for JSON and TEXT requests
impl ReqwestPlugin {
    /// despawns finished reqwests if marked to be despawned
    fn remove_finished_requests(
        mut commands: Commands,
        q: Query<Entity, (With<DespawnReqwestEntity>, Without<ReqwestInflight>)>,
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
    ) {
        for (entity, mut request) in requests.iter_mut() {
            debug!("polling: {entity:?}");
            if let Some((result, parts)) = request.poll() {
                match result {
                    Ok(body) => {
                        // if the response is ok, the other values are already gotten, its safe to unwrap
                        let parts = parts.unwrap();

                        commands.trigger_targets(
                            ReqwestResponseEvent::new(body.clone(), parts.status, parts.headers),
                            entity.clone(),
                        );
                    }
                    Err(err) => {
                        commands.trigger_targets(ReqwestErrorEvent(err), entity.clone());
                    }
                }
                if let Some(mut ec) = commands.get_entity(entity) {
                    ec.remove::<ReqwestInflight>();
                }
            }
        }
    }
}

/// Wrapper around EntityCommands to create the on_response and on_error
pub struct BevyReqwestBuilder<'a>(EntityCommands<'a>);

impl<'a> BevyReqwestBuilder<'a> {
    /// Provide a system where the first argument is [`Trigger`] [`ReqwestResponseEvent`] that will run on the
    /// response from the http request
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy::prelude::Trigger;
    /// use bevy_mod_reqwest::ReqwestResponseEvent;
    /// |trigger: Trigger<ReqwestResponseEvent>|  {
    ///   bevy::log::info!("response: {:?}", trigger.event());
    /// };
    /// ```
    pub fn on_response<RB: Bundle, RM, OR: IntoObserverSystem<ReqwestResponseEvent, RB, RM>>(
        &mut self,
        onresponse: OR,
    ) -> &mut Self {
        self.0.observe(onresponse);
        self
    }

    /// Provide a system where the first argument is [`Trigger`] [`JsonResponse`] that will run on the
    /// response from the http request
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy::prelude::Trigger;
    /// use bevy_mod_reqwest::ReqwestResponseEvent;
    /// |trigger: Trigger<JsonResponse<T>|  {
    ///   bevy::log::info!("response: {:?}", trigger.event());
    /// };
    /// ```
    pub fn on_json_response<
        T: std::marker::Sync + std::marker::Send + serde::de::DeserializeOwned + 'static,
        RB: Bundle,
        RM,
        OR: IntoObserverSystem<json::JsonResponse<T>, RB, RM>,
    >(
        &mut self,
        onresponse: OR,
    ) -> &mut Self {
        self.0.observe(
            |evt: Trigger<ReqwestResponseEvent>, mut commands: Commands| {
                let entity = evt.entity();
                let evt = evt.event();
                let data = evt.deserialize_json::<T>();

                match data {
                    Ok(data) => {
                        // retrigger a new event with the serialized data
                        commands.trigger_targets(json::JsonResponse(data), entity);
                    }
                    Err(e) => {
                        bevy::log::error!("deserialization error: {e}");
                        bevy::log::debug!(
                            "tried serializing: {}",
                            evt.as_str().unwrap_or("failed getting event data")
                        );
                    }
                }
            },
        );
        self.0.observe(onresponse);
        self
    }

    /// Provide a system where the first argument is [`Trigger`] [`ReqwestErrorEvent`] that will run on the
    /// response from the http request
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy::prelude::Trigger;
    /// use bevy_mod_reqwest::ReqwestErrorEvent;
    /// |trigger: Trigger<ReqwestErrorEvent>|  {
    ///   bevy::log::info!("response: {:?}", trigger.event());
    /// };
    /// ```
    pub fn on_error<EB: Bundle, EM, OE: IntoObserverSystem<ReqwestErrorEvent, EB, EM>>(
        &mut self,
        onerror: OE,
    ) -> &mut Self {
        self.0.observe(onerror);
        self
    }
}

#[derive(SystemParam)]
/// Systemparam to have a shorthand for creating http calls in systems
pub struct BevyReqwest<'w, 's> {
    commands: Commands<'w, 's>,
    client: Res<'w, ReqwestClient>,
}

impl<'w, 's> BevyReqwest<'w, 's> {
    /// Starts sending and processing the supplied [`reqwest::Request`]
    /// then use the [`BevyReqwestBuilder`] to add handlers for responses and errors
    pub fn send(&mut self, req: reqwest::Request) -> BevyReqwestBuilder {
        let inflight = self.create_inflight_task(req);
        BevyReqwestBuilder(self.commands.spawn((inflight, DespawnReqwestEntity)))
    }

    /// Starts sending and processing the supplied [`reqwest::Request`] on the supplied [`Entity`] if it exists
    /// and then use the [`BevyReqwestBuilder`] to add handlers for responses and errors
    pub fn send_using_entity(
        &mut self,
        entity: Entity,
        req: reqwest::Request,
    ) -> Option<BevyReqwestBuilder> {
        let inflight = self.create_inflight_task(req);
        let mut ec = self.commands.get_entity(entity)?;
        info!("inserting request on entity: {:?}", entity);
        ec.insert(inflight);
        Some(BevyReqwestBuilder(ec))
    }

    /// get access to the underlying ReqwestClient
    pub fn client(&self) -> &reqwest::Client {
        &self.client.0
    }

    fn create_inflight_task(&self, request: reqwest::Request) -> ReqwestInflight {
        let thread_pool = IoTaskPool::get();
        // bevy::log::debug!("Creating: {entity:?}");
        // if we take the data, we can use it
        let client = self.client.0.clone();
        let url = request.url().to_string();

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
                let task_res = async_compat::Compat::new(async {
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
                task_res
            })
        };
        // put it as a component to be polled, and remove the request, it has been handled
        ReqwestInflight::new(task, url)
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
/// to be used inside the bevy application
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

type Resp = (reqwest::Result<bytes::Bytes>, Option<Parts>);

/// Dont touch these, its just to poll once every request
#[derive(Component)]
#[component(storage = "SparseSet")]
struct ReqwestInflight {
    // the url this request is handling as a string
    pub url: String,
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
    pub(crate) fn new(res: Receiver<Resp>, url: String) -> Self {
        Self { url, res }
    }

    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn new(res: Task<Resp>, url: String) -> Self {
        Self { url, res }
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

#[derive(Clone, Event, Debug)]
/// the resulting data from a finished request is found here
pub struct ReqwestResponseEvent {
    bytes: bytes::Bytes,
    status: StatusCode,
    headers: HeaderMap,
}

#[derive(Event, Debug)]
pub struct ReqwestErrorEvent(pub reqwest::Error);

impl ReqwestResponseEvent {
    /// retrieve a reference to the body of the response
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
    pub fn response_headers(&self) -> &HeaderMap {
        &self.headers
    }
}

#[cfg(feature = "json")]
pub mod json {
    use bevy::prelude::Event;
    use serde::Deserialize;
    #[derive(Deserialize, Event)]
    pub struct JsonResponse<T>(pub T);
}

impl ReqwestResponseEvent {
    pub(crate) fn new(bytes: bytes::Bytes, status: StatusCode, headers: HeaderMap) -> Self {
        Self {
            bytes,
            status,
            headers,
        }
    }
}
