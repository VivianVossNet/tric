// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Datagram codec — pure functions for decoding requests and encoding responses (local, unencrypted).

pub struct Request {
    pub request_id: u32,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

pub struct Response {
    pub request_id: u32,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

pub fn decode_local(raw: &[u8]) -> Option<Request> {
    if raw.len() < 5 {
        return None;
    }
    let request_id = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let opcode = raw[4];
    if opcode == 0x00 || opcode == 0xFF {
        return None;
    }
    let payload = raw[5..].to_vec();
    Some(Request {
        request_id,
        opcode,
        payload,
    })
}

pub fn encode_local(response: &Response) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(5 + response.payload.len());
    buffer.extend_from_slice(&response.request_id.to_be_bytes());
    buffer.push(response.opcode);
    buffer.extend_from_slice(&response.payload);
    buffer
}
