// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Integration tests for time-dependent TTL and lazy-expiry behaviour of `Tric`.

use std::thread;
use std::time::Duration;
use tric::{create_tric, Bytes};

#[test]
fn check_expired_key_is_unreadable() {
    let tric = create_tric();
    tric.write_value(b"ephemeral", b"fades");
    tric.write_ttl(b"ephemeral", Duration::from_millis(100));
    thread::sleep(Duration::from_millis(200));
    assert_eq!(tric.read_value(b"ephemeral"), None);
}

#[test]
fn check_write_value_clears_previous_ttl() {
    let tric = create_tric();
    tric.write_value(b"key", b"first");
    tric.write_ttl(b"key", Duration::from_millis(100));
    tric.write_value(b"key", b"second");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(tric.read_value(b"key"), Some(Bytes::from_static(b"second")));
}

#[test]
fn check_write_ttl_on_missing_key_leaves_no_phantom_state() {
    let tric = create_tric();
    tric.write_ttl(b"absent", Duration::from_millis(50));
    thread::sleep(Duration::from_millis(100));
    tric.write_value(b"absent", b"now_here");
    assert_eq!(
        tric.read_value(b"absent"),
        Some(Bytes::from_static(b"now_here"))
    );
}

#[test]
fn check_delete_value_removes_ttl_state_completely() {
    let tric = create_tric();
    tric.write_value(b"key", b"first");
    tric.write_ttl(b"key", Duration::from_millis(100));
    tric.delete_value(b"key");
    tric.write_value(b"key", b"second");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(tric.read_value(b"key"), Some(Bytes::from_static(b"second")));
}

#[test]
fn check_expired_keys_are_excluded_from_scan() {
    let tric = create_tric();
    tric.write_value(b"temp:1", b"a");
    tric.write_value(b"temp:2", b"b");
    tric.write_ttl(b"temp:1", Duration::from_millis(100));
    tric.write_ttl(b"temp:2", Duration::from_millis(100));
    thread::sleep(Duration::from_millis(200));
    let result = tric.find_by_prefix(b"temp:");
    assert!(result.is_empty());
}
