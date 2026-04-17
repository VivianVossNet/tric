// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Server module — binds UDS DGRAM + UDP sockets, spawns workers, routes requests via DataBus.

use std::net::UdpSocket;
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::core::data_bus::DataBus;
use crate::core::module::{Module, ModuleContext};
use crate::modules::auth::SessionTable;
use crate::modules::codec::{decode_local, decode_network, encode_local, encode_network, Response};
use crate::modules::router::dispatch_request;

const MAX_DATAGRAM: usize = 2048;
const ERROR_MALFORMED: u8 = 0xA1;

pub struct ServerConfig {
    pub local_path: String,
    pub udp_bind: String,
    pub max_sessions: usize,
}

pub struct ServerModule {
    config: ServerConfig,
    metrics: Arc<crate::modules::metrics::Metrics>,
}

pub fn create_server(
    config: ServerConfig,
    metrics: Arc<crate::modules::metrics::Metrics>,
) -> ServerModule {
    ServerModule { config, metrics }
}

impl Module for ServerModule {
    fn name(&self) -> &'static str {
        "server"
    }

    fn run(&self, context: ModuleContext) {
        let _ = std::fs::remove_file(&self.config.local_path);
        let local_socket = Arc::new(UnixDatagram::bind(&self.config.local_path).unwrap_or_else(
            |error| {
                panic!(
                    "failed to bind local socket {}: {error}",
                    self.config.local_path
                )
            },
        ));

        let udp_socket = Arc::new(
            UdpSocket::bind(&self.config.udp_bind).unwrap_or_else(|error| {
                panic!(
                    "failed to bind UDP socket {}: {error}",
                    self.config.udp_bind
                )
            }),
        );

        let session_table = Arc::new(crate::modules::auth::create_session_table(
            self.config.max_sessions,
        ));

        let worker_count = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4);

        let core_bus = context.core_bus.clone();
        core_bus.write_value(b"module:server", b"running");
        core_bus.write_ttl(b"module:server", Duration::from_secs(15));

        let local_workers = worker_count / 2;
        let network_workers = worker_count - local_workers;

        let mut handles = Vec::with_capacity(worker_count);

        for _ in 0..local_workers.max(1) {
            let socket = Arc::clone(&local_socket);
            let core_bus = context.core_bus.clone();
            let data_bus = Arc::clone(&context.data_bus);
            let metrics = Arc::clone(&self.metrics);
            handles.push(thread::spawn(move || {
                run_local_worker(&socket, &core_bus, &data_bus, &metrics);
            }));
        }

        for _ in 0..network_workers.max(1) {
            let socket = Arc::clone(&udp_socket);
            let core_bus = context.core_bus.clone();
            let data_bus = Arc::clone(&context.data_bus);
            let sessions = Arc::clone(&session_table);
            let metrics = Arc::clone(&self.metrics);
            handles.push(thread::spawn(move || {
                run_network_worker(&socket, &core_bus, &data_bus, &sessions, &metrics);
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }
    }
}

fn run_local_worker(
    socket: &UnixDatagram,
    core_bus: &crate::Tric,
    data_bus: &Arc<dyn DataBus>,
    metrics: &crate::modules::metrics::Metrics,
) {
    let mut buffer = [0u8; MAX_DATAGRAM];
    loop {
        core_bus.write_ttl(b"module:server", Duration::from_secs(15));
        let (length, peer) = match socket.recv_from(&mut buffer) {
            Ok(result) => result,
            Err(_) => continue,
        };
        let start = std::time::Instant::now();
        metrics.record_local_request();
        let request = match decode_local(&buffer[..length]) {
            Some(request) => request,
            None => {
                metrics.record_error();
                let error = encode_local(&create_error(0, ERROR_MALFORMED));
                let _ = socket.send_to_addr(&error, &peer);
                continue;
            }
        };
        let responses = dispatch_request(&request, data_bus);
        for response in &responses {
            let encoded = encode_local(response);
            let _ = socket.send_to_addr(&encoded, &peer);
        }
        metrics.record_latency(start);
    }
}

fn run_network_worker(
    socket: &UdpSocket,
    core_bus: &crate::Tric,
    data_bus: &Arc<dyn DataBus>,
    session_table: &SessionTable,
    metrics: &crate::modules::metrics::Metrics,
) {
    let mut buffer = [0u8; MAX_DATAGRAM];
    loop {
        core_bus.write_ttl(b"module:server", Duration::from_secs(15));
        let (length, peer) = match socket.recv_from(&mut buffer) {
            Ok(result) => result,
            Err(_) => continue,
        };

        let start = std::time::Instant::now();
        metrics.record_network_request();

        let (request, session_id) = match decode_network(&buffer[..length], session_table) {
            Some(result) => result,
            None => continue,
        };

        let responses = dispatch_request(&request, data_bus);
        for response in &responses {
            if let Some(encoded) = encode_network(response, &session_id, session_table) {
                let _ = socket.send_to(&encoded, peer);
            }
        }
        metrics.record_latency(start);
    }
}

fn create_error(request_id: u32, opcode: u8) -> Response {
    Response {
        request_id,
        opcode,
        payload: Vec::new(),
    }
}
