# goggin-rs-logger

`goggin-rs-logger` is a Rust logging helper crate built on the `log` facade.
It provides a colorized global logger, semantic status helpers, optional Axum
request/response middleware, and an optional browser-to-server relay for
Leptos/WASM applications.

## Install

Default native logger:

```toml
[dependencies]
goggin-rs-logger = { git = "https://github.com/joegoggin/goggin-rs-logger" }
```

With Axum request/response middleware and relay receiver:

```toml
[dependencies]
goggin-rs-logger = { git = "https://github.com/joegoggin/goggin-rs-logger", features = ["axum"] }
```

With Leptos/WASM browser relay emitter:

```toml
[dependencies]
goggin-rs-logger = { git = "https://github.com/joegoggin/goggin-rs-logger", features = ["leptos"] }
```

With both relay emitter and receiver APIs:

```toml
[dependencies]
goggin-rs-logger = { git = "https://github.com/joegoggin/goggin-rs-logger", features = ["axum", "leptos"] }
```

## Native logger

```rust
use goggin_rs_logger::{Logger, info, log_message, log_success};

fn main() {
    Logger::setup_logging("info", false);

    log_message("hello from goggin-rs-logger");
    info!("standard log facade message");
    log_success("startup complete");
}
```

Use `Logger::setup_logging_from_env()` to read `LOG_LEVEL` and `LOG_VERBOSE`
from the environment. `LOG_LEVEL` defaults to `info`; `LOG_VERBOSE` defaults to
verbose output.

## Axum middleware

Enable the `axum` feature to log HTTP requests and responses.

```rust
use axum::{Router, middleware, routing::get};
use goggin_rs_logger::{HttpLoggingConfig, Logger};

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .route_layer(middleware::from_fn_with_state(
            HttpLoggingConfig::default(),
            Logger::log_request_and_response,
        ))
}
```

`HttpLoggingConfig::default()` disables body logging, uses a 16 KB body limit,
and emits verbose output. Use `HttpLoggingConfig::new(body_enabled,
max_body_bytes, verbose)` to override those settings.

## Leptos/WASM browser emitter

Enable the `leptos` feature to relay browser logs through the `log` facade.

```rust
use goggin_rs_logger::{
    WebLoggerConfig, init_web_logging, init_web_logging_with_config,
};

pub fn install_browser_logger() -> Result<(), log::SetLoggerError> {
    init_web_logging("info")?;
    Ok(())
}

pub fn install_browser_logger_with_custom_endpoint() -> Result<(), log::SetLoggerError> {
    let config = WebLoggerConfig::new("debug").with_endpoint("/internal/web-log");
    init_web_logging_with_config(config)?;
    Ok(())
}
```

`WEB_LOG_LEVEL` takes precedence over the default level passed to
`init_web_logging` or `WebLoggerConfig::new`.

## Axum relay receiver

Enable the `axum` feature to receive browser relay payloads on the server.

```rust
use std::sync::Arc;

use axum::Router;
use goggin_rs_logger::{RelayLogSink, RelayReceiverState, relay_router};

fn app() -> Router {
    let sink: RelayLogSink = Arc::new(|line| println!("{line}"));
    let relay_state = RelayReceiverState::new(sink, false);

    Router::new().merge(relay_router(relay_state))
}
```

The default browser endpoint is `/_leptos/web-log`. `relay_router` accepts
`POST /_leptos/web-log` for that default path and `POST /` so applications can
mount the receiver router under a custom endpoint that matches
`WebLoggerConfig::with_endpoint`.

## Relay contract

Browser emitters post `RelayLogPayload` JSON:

```json
{
  "level": "info",
  "message": "settings saved",
  "target": "app::settings",
  "file": "src/settings.rs",
  "line": 42
}
```

`level` and `message` are required. `target`, `file`, and `line` are optional.
The server receiver formats each payload with the same native logger style and
passes every formatted line to the configured `RelayLogSink`.
