// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Transient key-value store — BTreeMap-backed storage with lazy TTL expiry.

use std::collections::BTreeMap;
use std::time::Instant;

use bytes::Bytes;

#[allow(dead_code)]
pub(crate) struct Store {
    data: BTreeMap<Bytes, Bytes>,
    expiry: BTreeMap<Instant, Vec<Bytes>>,
    ttl: BTreeMap<Bytes, Instant>,
}

#[allow(dead_code)]
pub(crate) fn create_store() -> Store {
    Store {
        data: BTreeMap::new(),
        expiry: BTreeMap::new(),
        ttl: BTreeMap::new(),
    }
}

impl Store {
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub(crate) fn read_value(&self, key: &[u8]) -> Option<Bytes> {
        self.data.get(key).cloned()
    }

    #[allow(dead_code)]
    pub(crate) fn write_value(&mut self, key: Bytes, value: Bytes) {
        self.delete_ttl(&key);
        self.data.insert(key, value);
    }

    #[allow(dead_code)]
    pub(crate) fn delete_entry(&mut self, key: &[u8]) {
        self.delete_ttl(key);
        self.data.remove(key);
    }

    #[allow(dead_code)]
    pub(crate) fn write_ttl(&mut self, key: Bytes, instant: Instant) {
        if !self.data.contains_key(&key) {
            return;
        }
        self.delete_ttl(&key);
        self.ttl.insert(key.clone(), instant);
        self.expiry.entry(instant).or_default().push(key);
    }

    #[allow(dead_code)]
    pub(crate) fn delete_entry_if_match(&mut self, key: &[u8], expected: &[u8]) -> bool {
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
}
