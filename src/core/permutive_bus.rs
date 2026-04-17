// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Permutive storage router — TTL presence routes to transient (BTreeMap), absence to persistent (SQLite).

use std::path::Path;
use std::time::Duration;

use bytes::Bytes;

use crate::core::data_bus::DataBus;
use crate::core::sqlite_bus::{create_sqlite_bus, SqliteBus};
use crate::{create_tric, Tric};

const CACHE_PROMOTION_SECONDS: u64 = 60;

pub struct PermutiveBus {
    transient: Tric,
    persistent: SqliteBus,
}

pub fn create_permutive_bus(sqlite_directory: &Path) -> PermutiveBus {
    PermutiveBus {
        transient: create_tric(),
        persistent: create_sqlite_bus(sqlite_directory),
    }
}

impl DataBus for PermutiveBus {
    fn read_value(&self, key: &[u8]) -> Option<Bytes> {
        if let Some(value) = self.transient.read_value(key) {
            return Some(value);
        }

        let value = self.persistent.read_value(key)?;
        self.transient.write_value(key, &value);
        self.transient
            .write_ttl(key, Duration::from_secs(CACHE_PROMOTION_SECONDS));
        Some(value)
    }

    fn write_value(&self, key: &[u8], value: &[u8]) {
        self.persistent.write_value(key, value);
        self.transient.delete_value(key);
    }

    fn delete_value(&self, key: &[u8]) {
        self.transient.delete_value(key);
        self.persistent.delete_value(key);
    }

    fn delete_value_if_match(&self, key: &[u8], expected: &[u8]) -> bool {
        let transient_match = self.transient.delete_value_if_match(key, expected);
        let persistent_match = self.persistent.delete_value_if_match(key, expected);
        transient_match || persistent_match
    }

    fn write_ttl(&self, key: &[u8], duration: Duration) {
        if self.persistent.read_value(key).is_some() {
            let value = self.persistent.read_value(key).unwrap();
            self.persistent.delete_value(key);
            self.transient.write_value(key, &value);
        }
        self.transient.write_ttl(key, duration);
    }

    fn read_ttl_remaining(&self, key: &[u8]) -> Option<Duration> {
        self.transient.read_ttl_remaining(key)
    }

    fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)> {
        let transient_results = self.transient.find_by_prefix(prefix);
        let persistent_results = self.persistent.find_by_prefix(prefix);

        let mut merged = transient_results;
        for (key, value) in persistent_results {
            if !merged.iter().any(|(existing_key, _)| existing_key == &key) {
                merged.push((key, value));
            }
        }
        merged.sort_by(|(a, _), (b, _)| a.cmp(b));
        merged
    }
}
