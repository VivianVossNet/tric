// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Server module — binds UDS DGRAM socket, spawns worker threads, handles request lifecycle.

use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::core::module::{Module, ModuleContext};
use crate::modules::codec::{decode_local, encode_local, Response};

const MAX_DATAGRAM: usize = 2048;
const ERROR_INVALID_OPCODE: u8 = 0xA2;
const ERROR_MALFORMED: u8 = 0xA1;
const RESPONSE_OK: u8 = 0x80;

pub struct ServerConfig {
    pub local_path: String,
}

pub struct ServerModule {
    config: ServerConfig,
}

pub fn create_server(config: ServerConfig) -> ServerModule {
    ServerModule { config }
}

impl Module for ServerModule {
    fn name(&self) -> &'static str {
        "server"
    }

    fn run(&self, context: ModuleContext) {
        let _ = std::fs::remove_file(&self.config.local_path);
        let socket = Arc::new(UnixDatagram::bind(&self.config.local_path).unwrap_or_else(
            |error| {
                panic!(
                    "failed to bind local socket {}: {error}",
                    self.config.local_path
                )
            },
        ));

        let worker_count = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4);

        let core_bus = context.core_bus.clone();
        core_bus.write_value(b"module:server", b"running");
        core_bus.write_ttl(b"module:server", Duration::from_secs(15));

        let mut handles = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let socket = Arc::clone(&socket);
            let core_bus = context.core_bus.clone();

            handles.push(thread::spawn(move || {
                let mut buffer = [0u8; MAX_DATAGRAM];
                loop {
                    core_bus.write_ttl(b"module:server", Duration::from_secs(15));

                    let received = socket.recv_from(&mut buffer);
                    let (length, peer) = match received {
                        Ok(result) => result,
                        Err(_) => continue,
                    };

                    let response = match decode_local(&buffer[..length]) {
                        Some(request) => create_placeholder_response(&request),
                        None => create_error_response(0, ERROR_MALFORMED),
                    };

                    let encoded = encode_local(&response);
                    let _ = socket.send_to_addr(&encoded, &peer);
                }
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }
    }
}

fn create_placeholder_response(request: &crate::modules::codec::Request) -> Response {
    let family = request.opcode >> 4;
    if family > 0x07 {
        return create_error_response(request.request_id, ERROR_INVALID_OPCODE);
    }

    match request.opcode {
        0x01..=0x06 | 0x13 => Response {
            request_id: request.request_id,
            opcode: RESPONSE_OK,
            payload: Vec::new(),
        },
        _ => create_error_response(request.request_id, ERROR_INVALID_OPCODE),
    }
}

fn create_error_response(request_id: u32, error_opcode: u8) -> Response {
    Response {
        request_id,
        opcode: error_opcode,
        payload: Vec::new(),
    }
}
