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
}
