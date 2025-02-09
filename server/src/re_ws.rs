// NOTE: rerun is migrating to a gRPC server impl in the future so this will
// need to be rewritten
use anyhow::{Context, Result};
use axum::{
    extract::{
        ConnectInfo, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use axum_extra::TypedHeader;
use re_log_encoding::decoder::Decoder;
use std::{collections::VecDeque, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, RwLock, mpsc::Sender};
use tracing::{debug, error, info, trace};

use futures_util::SinkExt;
use re_log_types::{BlueprintActivationCommand, LogMsg};
use re_memory::MemoryLimit;
use re_sdk::{RecordingStream, StoreKind};

#[derive(Clone)]
pub struct RerunState {
    pub recorder: RecordingStream,
    inner: RerunStateInner,
}

#[derive(Clone)]
pub struct RerunStateInner {
    runtime_handle: tokio::runtime::Handle,
    message_queue: Arc<RwLock<MessageQueue>>,
    clients: Arc<Mutex<Vec<Sender<Vec<u8>>>>>,
}

impl RerunState {
    pub fn new(runtime_handle: tokio::runtime::Handle, memory_limit: MemoryLimit) -> Self {
        let store_info = re_log_types::StoreInfo {
            application_id: re_sdk::ApplicationId("brainlesscar".to_string()),
            store_id: re_sdk::StoreId::from_string(
                re_sdk::StoreKind::Recording,
                "brainlesscar".to_string(),
            ),
            cloned_from: None,
            is_official_example: false,
            started: re_sdk::Time::now(),
            store_source: re_log_types::StoreSource::Unknown,
            store_version: None,
        };
        let batcher_config = re_sdk::log::ChunkBatcherConfig::default();

        let inner = RerunStateInner {
            message_queue: Arc::new(RwLock::new(MessageQueue::new(memory_limit))),
            clients: Default::default(),
            runtime_handle,
        };
        Self {
            inner: inner.clone(),
            recorder: RecordingStream::new(store_info, batcher_config, Box::new(inner))
                .expect("failed to create recording stream"),
        }
    }
}

impl re_sdk::sink::LogSink for RerunStateInner {
    fn send(&self, msg: LogMsg) {
        let clients = self.clients.clone();
        let message_queue = self.message_queue.clone();

        self.runtime_handle.spawn(async move {
            let clients = clients.lock().await;
            let data = encode_log_msg(&msg);

            let mut message_queue = message_queue.write().await;
            match msg {
                LogMsg::ArrowMsg(store_id, _) if store_id.kind != StoreKind::Blueprint => {
                    message_queue.push(data.clone())
                }
                _ => message_queue.push_static(data.clone()),
            }
            let send_futures = clients.iter().map(|client| {
                let data = data.clone();
                async move {
                    if let Err(err) = client.send(data).await {
                        error!("Failed to send log message to client server: {err}");
                    }
                }
            });

            // Run all send operations in parallel
            futures_util::future::join_all(send_futures).await;
        });
    }

    #[inline]
    fn flush_blocking(&self) {
        let clients = self.clients.clone();

        self.runtime_handle.spawn(async move {
            let mut clients = clients.lock().await;
            let _ = clients.flush().await;
        });
    }
}

pub struct MessageQueue {
    server_memory_limit: MemoryLimit,
    messages: VecDeque<Vec<u8>>,

    /// Never garbage collected.
    messages_static: VecDeque<Vec<u8>>,
}

impl MessageQueue {
    pub fn new(memory_limit: MemoryLimit) -> Self {
        Self {
            server_memory_limit: memory_limit,
            messages: Default::default(),
            messages_static: Default::default(),
        }
    }

    fn push(&mut self, msg: Vec<u8>) {
        self.gc_if_using_too_much_ram();
        self.messages.push_back(msg);
    }

    /// static messages are never dropped
    fn push_static(&mut self, msg: Vec<u8>) {
        self.gc_if_using_too_much_ram();
        self.messages_static.push_back(msg);
    }

    fn gc_if_using_too_much_ram(&mut self) {
        let bytes_used = self.messages.iter().map(|m| m.len() as u64).sum::<u64>();
        if let Some(max_bytes) = self.server_memory_limit.max_bytes {
            let max_bytes = max_bytes as u64;
            if max_bytes < bytes_used {
                info!(
                    "Memory limit ({}) exceeded. Dropping old log messages from the server. Clients connecting after this will not see the full history.",
                    max_bytes
                );

                let bytes_to_free = bytes_used - max_bytes;

                let mut bytes_dropped = 0;
                let mut messages_dropped = 0;

                while bytes_dropped < bytes_to_free {
                    if let Some(msg) = self.messages.pop_front() {
                        bytes_dropped += msg.len() as u64;
                        messages_dropped += 1;
                    } else {
                        break;
                    }
                }

                trace!(
                    "Dropped {} bytes in {messages_dropped} message(s)",
                    bytes_dropped
                );
            }
        }
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<crate::ServerState>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    trace!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn handle_socket(mut socket: WebSocket, addr: SocketAddr, state: crate::ServerState) {
    // Create a dedicated channel for sending new events to this client.
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    {
        let mut clients = state.rerun.inner.clients.lock().await;
        clients.push(tx.clone());
    }
    // When a new client connects, first send all previous events.
    {
        let message_queue = state.rerun.inner.message_queue.read().await;
        for msg in &message_queue.messages_static {
            if (socket.send(Message::Binary(msg.clone().into())).await).is_err() {
                debug!("failed to send message to client {addr}");
            };
        }
        for msg in &message_queue.messages {
            if (socket.send(Message::Binary(msg.clone().into())).await).is_err() {
                debug!("failed to send message to client {addr}");
            };
        }
    }
    // Spawn a task to forward messages from the channel to the websocket.
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if socket.send(Message::binary::<Vec<u8>>(msg)).await.is_err() {
                {
                    // Remove the client if the connection closes
                    let mut clients = state.rerun.inner.clients.lock().await;
                    clients.retain(|sender| !sender.same_channel(&tx));
                }
                break;
            }
        }
        info!("Client disconnected {addr}");
    });
}

pub fn encode_log_msg(log_msg: &LogMsg) -> Vec<u8> {
    // rerun specific prefix
    const PREFIX: [u8; 4] = *b"RR00";

    use bincode::Options as _;
    let mut bytes = PREFIX.to_vec();
    bincode::DefaultOptions::new()
        .serialize_into(&mut bytes, log_msg)
        .unwrap();
    bytes
}

// gets blueprint from disk
pub fn get_blueprint(
    // store_info: re_log_types::StoreInfo,
    filepath: &std::path::Path,
) -> Result<(Vec<LogMsg>, BlueprintActivationCommand)> {
    // files are sometimes read on UI update).
    let file = std::fs::File::open(filepath)
        .with_context(|| format!("Failed to open file {filepath:?}"))?;
    let file = std::io::BufReader::new(file);

    let decoder = Decoder::new(re_log_encoding::VersionPolicy::Error, file)?;

    let mut messages = vec![];
    let mut activation_command: Option<BlueprintActivationCommand> = None;
    for msg in decoder {
        // info!("{msg:?}");
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                error!("Failed to decode message in {filepath:?}: {err}");
                continue;
            }
        };
        match msg {
            LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
                activation_command = Some(blueprint_activation_command)
            }
            _ => messages.push(msg),
        }
    }
    Ok((
        messages,
        activation_command.context("no activation command found in blueprint file")?,
    ))
}
