// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: DataBus trait and TricBus implementation — pluggable storage abstraction for the Core.

use std::time::Duration;

use crate::{create_tric, Tric};
use bytes::Bytes;

#[allow(dead_code)]
pub trait DataBus: Send + Sync {
    fn read_value(&self, key: &[u8]) -> Option<Bytes>;
    fn write_value(&self, key: &[u8], value: &[u8]);
    fn delete_value(&self, key: &[u8]);
    fn delete_value_if_match(&self, key: &[u8], expected: &[u8]) -> bool;
    fn write_ttl(&self, key: &[u8], duration: Duration);
    fn read_ttl_remaining(&self, key: &[u8]) -> Option<Duration>;
    fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)>;
}

pub struct TricBus {
    inner: Tric,
}

pub fn create_tric_bus() -> TricBus {
    TricBus {
        inner: create_tric(),
    }
}

impl DataBus for TricBus {
    fn read_value(&self, key: &[u8]) -> Option<Bytes> {
        self.inner.read_value(key)
    }

    fn write_value(&self, key: &[u8], value: &[u8]) {
        self.inner.write_value(key, value);
    }

    fn delete_value(&self, key: &[u8]) {
        self.inner.delete_value(key);
    }

    fn delete_value_if_match(&self, key: &[u8], expected: &[u8]) -> bool {
        self.inner.delete_value_if_match(key, expected)
    }

    fn write_ttl(&self, key: &[u8], duration: Duration) {
        self.inner.write_ttl(key, duration);
    }

    fn read_ttl_remaining(&self, key: &[u8]) -> Option<Duration> {
        self.inner.read_ttl_remaining(key)
    }

    fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)> {
        self.inner.find_by_prefix(prefix)
    }
}
