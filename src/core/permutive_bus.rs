// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Permutive storage router — TTL presence routes to transient (BTreeMap), absence to persistent (SQLite).

use std::path::{Path, PathBuf};
use std::time::Duration;

use bytes::Bytes;

use crate::core::data_bus::DataBus;
use crate::core::sqlite_bus::{create_sqlite_bus, find_instance_slots, SqliteBus};
use crate::{create_tric, Tric};

const CACHE_PROMOTION_SECONDS: u64 = 60;

pub struct PermutiveBus {
    transient: Tric,
    persistent: SqliteBus,
    base_dir: PathBuf,
    instance: String,
    slot: u32,
}

pub fn create_permutive_bus(base_dir: &Path, instance: &str, slot: u32) -> PermutiveBus {
    let transient = create_tric();
    let persistent = create_sqlite_bus(base_dir, instance, slot);

    let bus = PermutiveBus {
        transient,
        persistent,
        base_dir: base_dir.to_path_buf(),
        instance: instance.to_string(),
        slot,
    };

    bus.write_registry();
    bus
}

impl PermutiveBus {
    fn write_registry(&self) {
        let slots = find_instance_slots(&self.base_dir, &self.instance);
        for (slot_id, _) in &slots {
            let key = format!("_instance:{}_{slot_id}", self.instance);
            if *slot_id == 0 {
                self.transient.write_value(key.as_bytes(), b"active");
            } else {
                let origin = format!("clone:{}_{}", self.instance, 0);
                self.transient
                    .write_value(key.as_bytes(), origin.as_bytes());
            }
        }
    }

    pub fn read_base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn read_instance(&self) -> &str {
        &self.instance
    }

    pub fn read_slot(&self) -> u32 {
        self.slot
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
        if let Some(value) = self.persistent.read_value(key) {
            self.persistent.delete_value(key);
            self.transient.write_value(key, &value);
        }
        self.transient.write_ttl(key, duration);
    }

    fn write_value_with_ttl(&self, key: &[u8], value: &[u8], duration: Duration) {
        self.persistent.delete_value(key);
        self.transient.write_value_with_ttl(key, value, duration);
    }

    fn read_ttl_remaining(&self, key: &[u8]) -> Option<Duration> {
        self.transient.read_ttl_remaining(key)
    }

    fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)> {
        let transient_results = self.transient.find_by_prefix(prefix);
        let persistent_results = self.persistent.find_by_prefix(prefix);

        let transient_keys: std::collections::HashSet<Bytes> = transient_results
            .iter()
            .map(|(key, _)| key.clone())
            .collect();
        let mut merged = transient_results;
        for (key, value) in persistent_results {
            if !transient_keys.contains(&key) {
                merged.push((key, value));
            }
        }
        merged.sort_by(|(a, _), (b, _)| a.cmp(b));
        merged
    }
}
