// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Transient key-value store — BTreeMap-backed storage with lazy TTL expiry.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use bytes::Bytes;

pub(crate) struct Store {
    data: BTreeMap<Bytes, Bytes>,
    expiry: BTreeMap<Instant, Vec<Bytes>>,
    ttl: BTreeMap<Bytes, Instant>,
}

pub(crate) fn create_store() -> Store {
    Store {
        data: BTreeMap::new(),
        expiry: BTreeMap::new(),
        ttl: BTreeMap::new(),
    }
}

impl Store {
    pub(crate) fn delete_expired_entries(&mut self, now: Instant) {
        while let Some(exp) = self.expiry.range(..=now).next().map(|(&k, _)| k) {
            let keys = self.expiry.remove(&exp).unwrap();
            for key in &keys {
                self.ttl.remove(key);
                self.data.remove(key);
            }
        }
    }

    fn delete_ttl(&mut self, key: &[u8]) {
        let Some(instant) = self.ttl.remove(key) else {
            return;
        };
        let keys = self.expiry.get_mut(&instant).unwrap();
        let index = keys
            .iter()
            .position(|candidate| candidate.as_ref() == key)
            .unwrap();
        keys.swap_remove(index);
        if keys.is_empty() {
            self.expiry.remove(&instant);
        }
    }

    pub(crate) fn read_value(&self, key: &[u8]) -> Option<Bytes> {
        self.data.get(key).cloned()
    }

    pub(crate) fn write_value(&mut self, key: Bytes, value: Bytes) {
        self.delete_ttl(&key);
        self.data.insert(key, value);
    }

    pub(crate) fn delete_value(&mut self, key: &[u8]) {
        self.delete_ttl(key);
        self.data.remove(key);
    }

    pub(crate) fn write_ttl(&mut self, key: Bytes, instant: Instant) {
        if !self.data.contains_key(&key) {
            return;
        }
        self.delete_ttl(&key);
        self.ttl.insert(key.clone(), instant);
        self.expiry.entry(instant).or_default().push(key);
    }

    pub(crate) fn delete_value_if_match(&mut self, key: &[u8], expected: &[u8]) -> bool {
        if self
            .data
            .get(key)
            .is_some_and(|stored| stored.as_ref() == expected)
        {
            self.delete_ttl(key);
            self.data.remove(key);
            true
        } else {
            false
        }
    }

    pub(crate) fn read_ttl_remaining(&self, key: &[u8], now: Instant) -> Option<Duration> {
        let expiry_instant = self.ttl.get(key)?;
        expiry_instant.checked_duration_since(now)
    }

    pub(crate) fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)> {
        let start = Bytes::copy_from_slice(prefix);
        self.data
            .range(start..)
            .take_while(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}
