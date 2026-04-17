// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: TRIC+ Permutive Database Engine — public API, storage core, server kernel.

pub mod core;
pub mod modules;
mod store;

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub use bytes::Bytes;

use store::{create_store, Store};

#[derive(Clone)]
pub struct Tric {
    inner: Arc<RwLock<Store>>,
}

pub fn create_tric() -> Tric {
    Tric {
        inner: Arc::new(RwLock::new(create_store())),
    }
}

impl Tric {
    pub fn read_value(&self, key: &[u8]) -> Option<Bytes> {
        let mut store = self.inner.write().unwrap();
        store.delete_expired_entries(Instant::now());
        store.read_value(key)
    }

    pub fn write_value(&self, key: &[u8], value: &[u8]) {
        let mut store = self.inner.write().unwrap();
        store.delete_expired_entries(Instant::now());
        store.write_value(Bytes::copy_from_slice(key), Bytes::copy_from_slice(value));
    }

    pub fn delete_value(&self, key: &[u8]) {
        let mut store = self.inner.write().unwrap();
        store.delete_expired_entries(Instant::now());
        store.delete_value(key);
    }

    pub fn delete_value_if_match(&self, key: &[u8], expected: &[u8]) -> bool {
        let mut store = self.inner.write().unwrap();
        store.delete_expired_entries(Instant::now());
        store.delete_value_if_match(key, expected)
    }

    pub fn write_ttl(&self, key: &[u8], duration: Duration) {
        let mut store = self.inner.write().unwrap();
        let now = Instant::now();
        store.delete_expired_entries(now);
        store.write_ttl(Bytes::copy_from_slice(key), now + duration);
    }

    pub fn read_ttl_remaining(&self, key: &[u8]) -> Option<Duration> {
        let mut store = self.inner.write().unwrap();
        let now = Instant::now();
        store.delete_expired_entries(now);
        store.read_ttl_remaining(key, now)
    }

    pub fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)> {
        let mut store = self.inner.write().unwrap();
        store.delete_expired_entries(Instant::now());
        store.find_by_prefix(prefix)
    }
}
