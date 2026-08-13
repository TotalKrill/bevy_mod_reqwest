# bevy_mod_reqwest Testing

Run native tests, example compilation, and smoke-run the native examples first. The examples run forever, so use `timeout`; exit code 124 is expected as long as logs show callbacks firing.

```sh
cargo fmt --check
cargo test
cargo check --examples
./scripts/smoke-native-examples.sh
```

The examples default to the local `http://127.0.0.1:8090` test server. The `scripts/cors-test-server.rs` Axum cargo script is the reliable local test endpoint for both native and browser/wasm runs. It serves CORS-enabled `GET /random` and `POST /posts` responses. It requires the nightly toolchain because it uses `cargo +nightly -Zscript`. Use `BEVY_MOD_REQWEST_EXAMPLE_BASE_URL` only when intentionally pointing examples at a different server.

Also verify wasm compilation at minimum:

```sh
rustup target add wasm32-unknown-unknown
cargo check --target wasm32-unknown-unknown --examples
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-server-runner \
  cargo test --target wasm32-unknown-unknown --lib --no-run
```

For browser wasm runs, use `wasm-server-runner` plus Chromium DevTools/browser console. `wasm-server-runner` must use a `wasm-bindgen-cli-support` version matching this crate's locked `wasm-bindgen` version in `Cargo.lock`.

Typical install/update flow:

```sh
rg 'name = "wasm-bindgen"' Cargo.lock -A2
rm -rf /tmp/wasm-server-runner
git clone https://github.com/jakobhellermann/wasm-server-runner /tmp/wasm-server-runner
# Edit /tmp/wasm-server-runner/Cargo.toml:
# wasm-bindgen-cli-support = "=<matching wasm-bindgen version>"
cd /tmp/wasm-server-runner
cargo update -p wasm-bindgen-cli-support --precise <matching wasm-bindgen version>
cargo install --path . --locked
```

When actually running wasm examples/tests, use the scripted Chromium smoke test:

```sh
./scripts/smoke-wasm-example.sh
```

It starts the local CORS server, runs the wasm `minimal` example with `wasm-server-runner` on a non-blocked port, launches Chromium with remote debugging, and checks browser console logs for successful callbacks such as `code: 200 OK` and `local bevy_mod_reqwest CORS test server`.
