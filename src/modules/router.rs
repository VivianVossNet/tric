// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Router — dispatches decoded datagram opcodes to DataBus methods, builds responses.

use std::sync::Arc;
use std::time::Duration;

use crate::core::data_bus::DataBus;
use crate::modules::codec::{Request, Response};
use crate::modules::metrics::Metrics;

const OK: u8 = 0x80;
const OK_PAYLOAD: u8 = 0x81;
const SCAN_CHUNK: u8 = 0x90;
const SCAN_END: u8 = 0x91;
const ERROR_MALFORMED: u8 = 0xA1;
const ERROR_INVALID_OPCODE: u8 = 0xA2;

pub fn dispatch_request(
    request: &Request,
    data_bus: &Arc<dyn DataBus>,
    metrics: &Metrics,
) -> Vec<Response> {
    match request.opcode {
        0x01 => vec![read_value(request, data_bus)],
        0x02 => vec![write_value(request, data_bus)],
        0x03 => vec![delete_value(request, data_bus)],
        0x04 => vec![delete_value_if_match(request, data_bus)],
        0x05 => vec![write_ttl(request, data_bus)],
        0x06 => find_by_prefix(request, data_bus),
        0x07 => dispatch_query(request, data_bus),
        0x13 => vec![create_ok(request.request_id)],
        0x14 => vec![read_status(request, metrics)],
        0x15 => vec![parse_shutdown(request)],
        0x16 => vec![parse_reload(request)],
        0x17 => read_keys(request, data_bus),
        0x18 => vec![read_inspect(request, data_bus)],
        0x19 => read_dump(request, data_bus),
        0x1A => vec![write_restore(request, data_bus)],
        _ => vec![create_error(request.request_id, ERROR_INVALID_OPCODE)],
    }
}

fn read_value(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some(key) = read_field(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    match data_bus.read_value(key) {
        Some(value) => {
            let mut payload = Vec::with_capacity(4 + value.len());
            payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
            payload.extend_from_slice(&value);
            Response {
                request_id: request.request_id,
                opcode: OK_PAYLOAD,
                payload,
            }
        }
        None => create_ok(request.request_id),
    }
}

fn write_value(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some((key, offset)) = read_field_with_offset(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    let Some(value) = read_field(&request.payload, offset) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    data_bus.write_value(key, value);
    create_ok(request.request_id)
}

fn delete_value(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some(key) = read_field(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    data_bus.delete_value(key);
    create_ok(request.request_id)
}

fn delete_value_if_match(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some((key, offset)) = read_field_with_offset(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    let Some(expected) = read_field(&request.payload, offset) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    let matched = data_bus.delete_value_if_match(key, expected);
    Response {
        request_id: request.request_id,
        opcode: OK_PAYLOAD,
        payload: vec![if matched { 0x01 } else { 0x00 }],
    }
}

fn write_ttl(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some((key, offset)) = read_field_with_offset(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    if request.payload.len() < offset + 8 {
        return create_error(request.request_id, ERROR_MALFORMED);
    }
    let duration_ms = u64::from_be_bytes([
        request.payload[offset],
        request.payload[offset + 1],
        request.payload[offset + 2],
        request.payload[offset + 3],
        request.payload[offset + 4],
        request.payload[offset + 5],
        request.payload[offset + 6],
        request.payload[offset + 7],
    ]);
    data_bus.write_ttl(key, Duration::from_millis(duration_ms));
    create_ok(request.request_id)
}

fn find_by_prefix(request: &Request, data_bus: &Arc<dyn DataBus>) -> Vec<Response> {
    let Some(prefix) = read_field(&request.payload, 0) else {
        return vec![create_error(request.request_id, ERROR_MALFORMED)];
    };
    let pairs = data_bus.find_by_prefix(prefix);
    let total = pairs.len().min(u16::MAX as usize) as u16;
    let mut responses = Vec::with_capacity(pairs.len().min(u16::MAX as usize) + 1);

    for (chunk_id, (key, value)) in pairs.iter().take(u16::MAX as usize).enumerate() {
        let mut payload = Vec::with_capacity(4 + key.len() + 4 + value.len() + 4);
        payload.extend_from_slice(&total.to_be_bytes());
        payload.extend_from_slice(&(chunk_id as u16).to_be_bytes());
        payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
        payload.extend_from_slice(key);
        payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
        payload.extend_from_slice(value);
        responses.push(Response {
            request_id: request.request_id,
            opcode: SCAN_CHUNK,
            payload,
        });
    }

    responses.push(Response {
        request_id: request.request_id,
        opcode: SCAN_END,
        payload: Vec::new(),
    });

    responses
}

fn dispatch_query(request: &Request, data_bus: &Arc<dyn DataBus>) -> Vec<Response> {
    let Some(sql_bytes) = read_field(&request.payload, 0) else {
        return vec![create_error(request.request_id, ERROR_MALFORMED)];
    };
    let sql = String::from_utf8_lossy(sql_bytes);
    crate::modules::query::parse_query(&sql, request.request_id, data_bus)
}

fn read_field(payload: &[u8], offset: usize) -> Option<&[u8]> {
    read_field_with_offset(payload, offset).map(|(field, _)| field)
}

fn read_field_with_offset(payload: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    if payload.len() < offset + 4 {
        return None;
    }
    let length = u32::from_be_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ]) as usize;
    let field_start = offset + 4;
    let field_end = field_start + length;
    if payload.len() < field_end {
        return None;
    }
    Some((&payload[field_start..field_end], field_end))
}

fn read_status(request: &Request, metrics: &Metrics) -> Response {
    if !request.is_local {
        return create_error(request.request_id, ERROR_INVALID_OPCODE);
    }
    let mut payload = Vec::with_capacity(56);
    payload.extend_from_slice(&metrics.read_requests_total().to_be_bytes());
    payload.extend_from_slice(&metrics.read_requests_local().to_be_bytes());
    payload.extend_from_slice(&metrics.read_requests_network().to_be_bytes());
    payload.extend_from_slice(&metrics.read_errors_total().to_be_bytes());
    payload.extend_from_slice(&metrics.read_active_sessions().to_be_bytes());
    payload.extend_from_slice(&metrics.read_latency_average_microseconds().to_be_bytes());
    payload.extend_from_slice(&metrics.read_latency_max_microseconds().to_be_bytes());
    Response {
        request_id: request.request_id,
        opcode: OK_PAYLOAD,
        payload,
    }
}

fn parse_shutdown(request: &Request) -> Response {
    if !request.is_local {
        return create_error(request.request_id, ERROR_INVALID_OPCODE);
    }
    crate::modules::logger::log_info("shutdown requested via binary protocol");
    std::process::exit(0);
}

fn parse_reload(request: &Request) -> Response {
    if !request.is_local {
        return create_error(request.request_id, ERROR_INVALID_OPCODE);
    }
    crate::modules::logger::log_info("reload requested via binary protocol");
    create_ok(request.request_id)
}

fn read_keys(request: &Request, data_bus: &Arc<dyn DataBus>) -> Vec<Response> {
    let prefix = read_field(&request.payload, 0).unwrap_or(b"");
    let pairs = data_bus.find_by_prefix(prefix);
    let total = pairs.len().min(u16::MAX as usize) as u16;
    let mut responses = Vec::with_capacity(pairs.len().min(u16::MAX as usize) + 1);

    for (chunk_id, (key, value)) in pairs.iter().take(u16::MAX as usize).enumerate() {
        let mut payload = Vec::with_capacity(4 + key.len() + 4 + value.len() + 4);
        payload.extend_from_slice(&total.to_be_bytes());
        payload.extend_from_slice(&(chunk_id as u16).to_be_bytes());
        payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
        payload.extend_from_slice(key);
        payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
        payload.extend_from_slice(value);
        responses.push(Response {
            request_id: request.request_id,
            opcode: SCAN_CHUNK,
            payload,
        });
    }

    responses.push(Response {
        request_id: request.request_id,
        opcode: SCAN_END,
        payload: Vec::new(),
    });

    responses
}

fn read_inspect(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some(key) = read_field(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    match data_bus.read_value(key) {
        Some(value) => {
            let ttl_ms = data_bus
                .read_ttl_remaining(key)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0);
            let mut payload = Vec::with_capacity(4 + value.len() + 8);
            payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
            payload.extend_from_slice(&value);
            payload.extend_from_slice(&ttl_ms.to_be_bytes());
            Response {
                request_id: request.request_id,
                opcode: OK_PAYLOAD,
                payload,
            }
        }
        None => create_ok(request.request_id),
    }
}

fn read_dump(request: &Request, data_bus: &Arc<dyn DataBus>) -> Vec<Response> {
    let pairs = data_bus.find_by_prefix(b"");
    let total = pairs.len().min(u16::MAX as usize) as u16;
    let mut responses = Vec::with_capacity(pairs.len().min(u16::MAX as usize) + 1);

    for (chunk_id, (key, value)) in pairs.iter().take(u16::MAX as usize).enumerate() {
        let ttl_ms = data_bus
            .read_ttl_remaining(key)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        let mut payload = Vec::with_capacity(4 + key.len() + 4 + value.len() + 8 + 4);
        payload.extend_from_slice(&total.to_be_bytes());
        payload.extend_from_slice(&(chunk_id as u16).to_be_bytes());
        payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
        payload.extend_from_slice(key);
        payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
        payload.extend_from_slice(value);
        payload.extend_from_slice(&ttl_ms.to_be_bytes());
        responses.push(Response {
            request_id: request.request_id,
            opcode: SCAN_CHUNK,
            payload,
        });
    }

    responses.push(Response {
        request_id: request.request_id,
        opcode: SCAN_END,
        payload: Vec::new(),
    });

    responses
}

fn write_restore(request: &Request, data_bus: &Arc<dyn DataBus>) -> Response {
    let Some((key, offset)) = read_field_with_offset(&request.payload, 0) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    let Some((value, offset)) = read_field_with_offset(&request.payload, offset) else {
        return create_error(request.request_id, ERROR_MALFORMED);
    };
    data_bus.write_value(key, value);
    if request.payload.len() >= offset + 8 {
        let ttl_ms = u64::from_be_bytes([
            request.payload[offset],
            request.payload[offset + 1],
            request.payload[offset + 2],
            request.payload[offset + 3],
            request.payload[offset + 4],
            request.payload[offset + 5],
            request.payload[offset + 6],
            request.payload[offset + 7],
        ]);
        if ttl_ms > 0 {
            data_bus.write_ttl(key, Duration::from_millis(ttl_ms));
        }
    }
    create_ok(request.request_id)
}

fn create_ok(request_id: u32) -> Response {
    Response {
        request_id,
        opcode: OK,
        payload: Vec::new(),
    }
}

fn create_error(request_id: u32, error_opcode: u8) -> Response {
    Response {
        request_id,
        opcode: error_opcode,
        payload: Vec::new(),
    }
}
