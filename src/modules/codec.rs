// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Datagram codec — encode/decode for local (unencrypted) and network (encrypted + noise).

use crate::modules::auth::SessionTable;
use rand_core::{OsRng, RngCore};

pub struct Request {
    pub request_id: u32,
    pub opcode: u8,
    pub payload: Vec<u8>,
    pub is_local: bool,
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
        is_local: true,
    })
}

pub fn encode_local(response: &Response) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(5 + response.payload.len());
    buffer.extend_from_slice(&response.request_id.to_be_bytes());
    buffer.push(response.opcode);
    buffer.extend_from_slice(&response.payload);
    buffer
}

pub fn decode_network(raw: &[u8], session_table: &SessionTable) -> Option<(Request, [u8; 16])> {
    if raw.len() < 16 + 12 + 5 + 16 {
        return None;
    }
    let mut session_id = [0u8; 16];
    session_id.copy_from_slice(&raw[..16]);

    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&raw[16..28]);

    let ciphertext = &raw[28..];
    let plaintext = session_table.decrypt_request(&session_id, ciphertext, &nonce)?;

    if plaintext.len() < 5 {
        return None;
    }
    let request_id = u32::from_be_bytes([plaintext[0], plaintext[1], plaintext[2], plaintext[3]]);
    let opcode = plaintext[4];
    if opcode == 0x00 || opcode == 0xFF {
        return None;
    }

    let payload = extract_real_payload(opcode, &plaintext[5..]);

    Some((
        Request {
            request_id,
            opcode,
            payload,
            is_local: false,
        },
        session_id,
    ))
}

pub fn encode_network(
    response: &Response,
    session_id: &[u8; 16],
    session_table: &SessionTable,
) -> Option<Vec<u8>> {
    let mut plaintext = Vec::with_capacity(5 + response.payload.len() + 512);
    plaintext.extend_from_slice(&response.request_id.to_be_bytes());
    plaintext.push(response.opcode);
    plaintext.extend_from_slice(&response.payload);

    let noise_length = 64 + (OsRng.next_u32() as usize % 961);
    let mut noise = vec![0u8; noise_length];
    OsRng.fill_bytes(&mut noise);
    plaintext.extend_from_slice(&noise);

    let encrypted = session_table.encrypt_response(session_id, &plaintext)?;

    let mut datagram = Vec::with_capacity(16 + 12 + encrypted.len());
    datagram.extend_from_slice(session_id);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    datagram.extend_from_slice(&nonce_bytes);
    datagram.extend_from_slice(&encrypted);

    Some(datagram)
}

fn extract_real_payload(opcode: u8, after_opcode: &[u8]) -> Vec<u8> {
    let consumed = match opcode {
        0x01 | 0x03 => read_one_field_length(after_opcode),
        0x02 => read_two_field_lengths_plus_u64(after_opcode),
        0x04 => read_two_field_lengths(after_opcode),
        0x05 => read_one_field_length_plus_u64(after_opcode),
        0x06 => read_one_field_length(after_opcode),
        0x13 => Some(0),
        0x10 => Some(64),
        0x11 => Some(64),
        _ => None,
    };
    match consumed {
        Some(length) if length <= after_opcode.len() => after_opcode[..length].to_vec(),
        _ => after_opcode.to_vec(),
    }
}

fn read_one_field_length(data: &[u8]) -> Option<usize> {
    if data.len() < 4 {
        return None;
    }
    let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    Some(4 + length)
}

fn read_two_field_lengths(data: &[u8]) -> Option<usize> {
    let first = read_one_field_length(data)?;
    if data.len() < first + 4 {
        return None;
    }
    let second_length = u32::from_be_bytes([
        data[first],
        data[first + 1],
        data[first + 2],
        data[first + 3],
    ]) as usize;
    Some(first + 4 + second_length)
}

fn read_one_field_length_plus_u64(data: &[u8]) -> Option<usize> {
    let first = read_one_field_length(data)?;
    Some(first + 8)
}

fn read_two_field_lengths_plus_u64(data: &[u8]) -> Option<usize> {
    let two_fields = read_two_field_lengths(data)?;
    Some(two_fields + 8)
}
