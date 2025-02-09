mod frame;
mod re_ws;

use anyhow::{Context, Result};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use axum::{Router, routing::any};
use tokio::runtime::Handle;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use re_memory::MemoryLimit;
use re_ws::RerunState;

use frame::{CameraSettings, FrameCapture, LidarSettings};

#[derive(Clone)]
pub struct ServerState {
    rerun: re_ws::RerunState,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let assets_dir = PathBuf::from(env!("FRONTEND_STATIC_DIR"));

    let server_state = ServerState {
        rerun: RerunState::new(Handle::current(), MemoryLimit::from_fraction_of_total(0.25)),
    };

    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/rerun", any(re_ws::ws_handler))
        // .route("/command", any(ws_handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        )
        .with_state(server_state.clone());

    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", env!("SERVER_ADDR"), env!("SERVER_PORT")))
            .await
            .unwrap();

    info!("Now listening on http://{}", listener.local_addr().unwrap());

    let frame_capture = FrameCapture::new(CameraSettings::default(), LidarSettings)?;

    // go until something fails
    tokio::select! {
        res = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()) => {
            res.context("axum server has stopped")
        },
        res = process_data(server_state, frame_capture) => {
            res.context("process_data has stopped")
        },
    }
}

async fn process_data(state: ServerState, mut frame_capture: FrameCapture) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let rec = state.rerun.recorder;

        // send the default blueprint
        let (blueprint, activation_command) =
            re_ws::get_blueprint(Path::new(env!("RERUN_BLUEPRINT_PATH")))?;
        rec.send_blueprint(blueprint.clone(), activation_command.clone());

        loop {
            frame_capture.fetch_frame()?;
            frame_capture.process_frame()?;
            frame_capture.log(&rec)?;
        }
    })
    .await?
}
