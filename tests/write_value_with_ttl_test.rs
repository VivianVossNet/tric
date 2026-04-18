// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Integration tests for the `write_value_with_ttl` primitive on the public `Tric` API.

use std::thread::sleep;
use std::time::Duration;
use tric::{create_tric, Bytes};

#[test]
fn check_write_value_with_ttl_sets_value_readable() {
    let tric = create_tric();
    tric.write_value_with_ttl(b"session", b"token", Duration::from_secs(60));
    assert_eq!(
        tric.read_value(b"session"),
        Some(Bytes::from_static(b"token"))
    );
}

#[test]
fn check_write_value_with_ttl_records_remaining_duration() {
    let tric = create_tric();
    tric.write_value_with_ttl(b"key", b"value", Duration::from_secs(120));
    let remaining = tric.read_ttl_remaining(b"key").unwrap();
    assert!(remaining <= Duration::from_secs(120));
    assert!(remaining > Duration::from_secs(119));
}

#[test]
fn check_write_value_with_ttl_replaces_existing_untimed_entry() {
    let tric = create_tric();
    tric.write_value(b"key", b"original");
    assert!(tric.read_ttl_remaining(b"key").is_none());
    tric.write_value_with_ttl(b"key", b"replaced", Duration::from_secs(60));
    assert_eq!(
        tric.read_value(b"key"),
        Some(Bytes::from_static(b"replaced"))
    );
    assert!(tric.read_ttl_remaining(b"key").is_some());
}

#[test]
fn check_write_value_with_ttl_overrides_previous_ttl() {
    let tric = create_tric();
    tric.write_value(b"key", b"value");
    tric.write_ttl(b"key", Duration::from_secs(3600));
    tric.write_value_with_ttl(b"key", b"renewed", Duration::from_secs(60));
    let remaining = tric.read_ttl_remaining(b"key").unwrap();
    assert!(remaining <= Duration::from_secs(60));
    assert_eq!(
        tric.read_value(b"key"),
        Some(Bytes::from_static(b"renewed"))
    );
}

#[test]
fn check_write_value_with_ttl_expires_after_duration() {
    let tric = create_tric();
    tric.write_value_with_ttl(b"brief", b"gone", Duration::from_millis(50));
    sleep(Duration::from_millis(120));
    assert_eq!(tric.read_value(b"brief"), None);
    assert!(tric.read_ttl_remaining(b"brief").is_none());
}
