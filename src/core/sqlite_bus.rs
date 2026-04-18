// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: SQLite persistent storage — scoped databases per namespace, DataBus implementation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use rusqlite::Connection;

use crate::core::data_bus::DataBus;

pub struct SqliteBus {
    directory: PathBuf,
    databases: Mutex<HashMap<String, Connection>>,
}

pub fn create_sqlite_bus(base_dir: &Path, instance: &str, slot: u32) -> SqliteBus {
    let directory = base_dir.join(format!("{instance}_{slot}"));
    std::fs::create_dir_all(&directory).unwrap_or_else(|error| {
        panic!(
            "failed to create SQLite directory {}: {error}",
            directory.display()
        )
    });

    let mut databases = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("db") {
                let namespace = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("_default")
                    .to_string();
                if let Ok(connection) = Connection::open(&path) {
                    let _ = connection.execute_batch(
                        "PRAGMA journal_mode=WAL;\
                         PRAGMA synchronous=NORMAL;\
                         CREATE TABLE IF NOT EXISTS data (\
                             key BLOB PRIMARY KEY,\
                             value BLOB NOT NULL\
                         );",
                    );
                    databases.insert(namespace, connection);
                }
            }
        }
    }

    SqliteBus {
        directory,
        databases: Mutex::new(databases),
    }
}

pub fn find_instance_slots(base_dir: &Path, instance: &str) -> Vec<(u32, u64)> {
    let mut slots = Vec::new();
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        let prefix = format!("{instance}_");
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(slot_str) = name.strip_prefix(&prefix) {
                if let Ok(slot) = slot_str.parse::<u32>() {
                    let path = entry.path();
                    let size = read_directory_size(&path);
                    slots.push((slot, size));
                }
            }
        }
    }
    slots.sort_by_key(|(slot, _)| *slot);
    slots
}

pub fn create_clone(
    base_dir: &Path,
    instance: &str,
    source_slot: u32,
    target_slot: u32,
) -> Result<u64, String> {
    let source = base_dir.join(format!("{instance}_{source_slot}"));
    let target = base_dir.join(format!("{instance}_{target_slot}"));

    if target.exists() {
        return Err(format!("slot {target_slot} already exists"));
    }
    if !source.exists() {
        return Err(format!("source slot {source_slot} does not exist"));
    }

    std::fs::create_dir_all(&target)
        .map_err(|error| format!("cannot create {target:?}: {error}"))?;

    let mut bytes_copied = 0u64;
    if let Ok(entries) = std::fs::read_dir(&source) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("db") {
                let dest = target.join(entry.file_name());
                if let Ok(metadata) = std::fs::copy(&path, &dest) {
                    bytes_copied += metadata;
                }
            }
        }
    }

    Ok(bytes_copied)
}

fn read_directory_size(path: &Path) -> u64 {
    let mut size = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                size += metadata.len();
            }
        }
    }
    size
}

impl SqliteBus {
    fn parse_namespace(key: &[u8]) -> (&[u8], String) {
        let key_str = std::str::from_utf8(key).unwrap_or("_default");
        let namespace = key_str
            .find(':')
            .map(|index| &key_str[..index])
            .unwrap_or("_default");
        (key, namespace.to_string())
    }

    fn read_connection(&self, namespace: &str) -> Option<()> {
        let databases = self.databases.lock().unwrap();
        if databases.contains_key(namespace) {
            Some(())
        } else {
            None
        }
    }

    fn write_connection(&self, namespace: &str) {
        let mut databases = self.databases.lock().unwrap();
        if databases.contains_key(namespace) {
            return;
        }
        let path = self.directory.join(format!("{namespace}.db"));
        let connection =
            Connection::open(&path).unwrap_or_else(|error| panic!("cannot open {path:?}: {error}"));
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;\
                 PRAGMA synchronous=NORMAL;\
                 CREATE TABLE IF NOT EXISTS data (\
                     key BLOB PRIMARY KEY,\
                     value BLOB NOT NULL\
                 );",
            )
            .unwrap_or_else(|error| panic!("cannot initialise {path:?}: {error}"));
        databases.insert(namespace.to_string(), connection);
    }
}

impl DataBus for SqliteBus {
    fn read_value(&self, key: &[u8]) -> Option<Bytes> {
        let (_, namespace) = Self::parse_namespace(key);
        self.read_connection(&namespace)?;
        let databases = self.databases.lock().unwrap();
        let connection = databases.get(&namespace)?;
        let mut statement = connection
            .prepare_cached("SELECT value FROM data WHERE key = ?1")
            .ok()?;
        statement
            .query_row([key], |row| {
                let value: Vec<u8> = row.get(0)?;
                Ok(Bytes::from(value))
            })
            .ok()
    }

    fn write_value(&self, key: &[u8], value: &[u8]) {
        let (_, namespace) = Self::parse_namespace(key);
        self.write_connection(&namespace);
        let databases = self.databases.lock().unwrap();
        let Some(connection) = databases.get(&namespace) else {
            return;
        };
        let _ = connection.execute(
            "INSERT OR REPLACE INTO data (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        );
    }

    fn delete_value(&self, key: &[u8]) {
        let (_, namespace) = Self::parse_namespace(key);
        if self.read_connection(&namespace).is_none() {
            return;
        }
        let databases = self.databases.lock().unwrap();
        let Some(connection) = databases.get(&namespace) else {
            return;
        };
        let _ = connection.execute("DELETE FROM data WHERE key = ?1", rusqlite::params![key]);
    }

    fn delete_value_if_match(&self, key: &[u8], expected: &[u8]) -> bool {
        let (_, namespace) = Self::parse_namespace(key);
        if self.read_connection(&namespace).is_none() {
            return false;
        }
        let databases = self.databases.lock().unwrap();
        let Some(connection) = databases.get(&namespace) else {
            return false;
        };
        let current: Option<Vec<u8>> = connection
            .prepare_cached("SELECT value FROM data WHERE key = ?1")
            .ok()
            .and_then(|mut statement| statement.query_row([key], |row| row.get(0)).ok());
        if current.as_deref() == Some(expected) {
            let _ = connection.execute("DELETE FROM data WHERE key = ?1", rusqlite::params![key]);
            true
        } else {
            false
        }
    }

    fn write_ttl(&self, _key: &[u8], _duration: Duration) {}

    fn write_value_with_ttl(&self, key: &[u8], value: &[u8], _duration: Duration) {
        self.write_value(key, value);
    }

    fn read_ttl_remaining(&self, _key: &[u8]) -> Option<Duration> {
        None
    }

    fn find_by_prefix(&self, prefix: &[u8]) -> Vec<(Bytes, Bytes)> {
        let databases = self.databases.lock().unwrap();
        let mut results = Vec::new();

        for connection in databases.values() {
            let Ok(mut statement) =
                connection.prepare_cached("SELECT key, value FROM data ORDER BY key")
            else {
                continue;
            };
            let Ok(rows) = statement.query_map([], |row| {
                let key: Vec<u8> = row.get(0)?;
                let value: Vec<u8> = row.get(1)?;
                Ok((Bytes::from(key), Bytes::from(value)))
            }) else {
                continue;
            };
            for row in rows.flatten() {
                if row.0.starts_with(prefix) {
                    results.push(row);
                }
            }
        }

        results.sort_by(|(a, _), (b, _)| a.cmp(b));
        results
    }
}
