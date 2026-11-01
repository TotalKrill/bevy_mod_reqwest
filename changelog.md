# Unreleased
 - Support bevy 0.20
 - `on_response`, `on_json_response` and `on_error` drop the `Bundle` generic parameter, matching Bevy 0.20's `IntoObserverSystem<E, M>`

# 0.22.1
 - Fix parameter-order-dependent crash when using `BevyReqwest::send_using_entity` with entities spawned via `Commands`
 - Add local native/wasm smoke-test scripts and CORS test server for examples

# 0.22.0
 - Support bevy 0.19

# 0.18
 - Support bevy 0.15
 - Added a `on_json_response` helper method to reduce boilerplate when using json API:s

# 0.17
 - Changed so that the request are polled in `PreUpdate` schedule

# Pre 0.17
 - changes not tracked
