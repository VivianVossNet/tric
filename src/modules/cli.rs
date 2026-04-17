// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: CLI module — admin socket listener, text command parser, FreeBSD-style responses.

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::time::Duration;

use crate::core::data_bus::DataBus;
use crate::core::module::{Module, ModuleContext};
use crate::modules::metrics::Metrics;

pub struct CliConfig {
    pub admin_path: String,
    pub auth_keys_path: Option<String>,
}

pub struct CliModule {
    config: CliConfig,
    metrics: Arc<Metrics>,
}

pub fn create_cli(config: CliConfig, metrics: Arc<Metrics>) -> CliModule {
    CliModule { config, metrics }
}

impl Module for CliModule {
    fn name(&self) -> &'static str {
        "cli"
    }

    fn run(&self, context: ModuleContext) {
        let _ = std::fs::remove_file(&self.config.admin_path);
        let socket = UnixDatagram::bind(&self.config.admin_path).unwrap_or_else(|error| {
            panic!(
                "failed to bind admin socket {}: {error}",
                self.config.admin_path
            )
        });

        let core_bus = context.core_bus.clone();
        let module_key = b"module:cli";
        core_bus.write_value(module_key, b"running");

        loop {
            core_bus.write_ttl(module_key, Duration::from_secs(15));

            let mut buffer = [0u8; 4096];
            let (length, peer) = match socket.recv_from(&mut buffer) {
                Ok(result) => result,
                Err(_) => continue,
            };

            let command = String::from_utf8_lossy(&buffer[..length]);
            let response = self.dispatch_command(command.trim(), &context.data_bus);
            let _ = socket.send_to_addr(response.as_bytes(), &peer);
        }
    }
}

impl CliModule {
    fn dispatch_command(&self, command: &str, data_bus: &Arc<dyn DataBus>) -> String {
        let mut parts = command.split_whitespace();
        let verb = parts.next().unwrap_or("");

        match verb {
            "status" => self.format_status(),
            "keys" => format_keys(parts, data_bus),
            "inspect" => format_inspect(parts, data_bus),
            "dump" => format_dump(parts, data_bus),
            "restore" => format_restore(parts, data_bus),
            "reload" => self.format_reload(),
            "shutdown" => format_shutdown(),
            "help" => format_help(),
            _ => format!("error: unknown command '{verb}'\n"),
        }
    }

    fn format_status(&self) -> String {
        format!(
            "tric-server\n  requests  {} total {} local {} network\n  errors    {}\n  sessions  {}\n  latency   {}us avg {}us max\n",
            self.metrics.read_requests_total(),
            self.metrics.read_requests_local(),
            self.metrics.read_requests_network(),
            self.metrics.read_errors_total(),
            self.metrics.read_active_sessions(),
            self.metrics.read_latency_average_microseconds(),
            self.metrics.read_latency_max_microseconds(),
        )
    }

    fn format_reload(&self) -> String {
        match &self.config.auth_keys_path {
            Some(path) => {
                crate::modules::logger::log_info("reload; source=authorized_keys trigger=admin");
                format!("reloaded {path}\n")
            }
            None => "auth disabled (--no-auth); nothing to reload\n".to_string(),
        }
    }
}

fn format_keys(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let prefix = match parts.next() {
        Some("-p") => parts.next().unwrap_or("").as_bytes(),
        _ => b"",
    };
    let pairs = data_bus.find_by_prefix(prefix);
    if pairs.is_empty() {
        return "(no keys)\n".to_string();
    }
    let mut output = String::with_capacity(pairs.len() * 40);
    for (key, value) in &pairs {
        output.push_str(&format!(
            "{}  {}B\n",
            String::from_utf8_lossy(key),
            value.len()
        ));
    }
    output
}

fn format_inspect(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let Some(key_str) = parts.next() else {
        return "usage: inspect <key>\n".to_string();
    };
    let key = key_str.as_bytes();
    match data_bus.read_value(key) {
        Some(value) => {
            let ttl_info = match data_bus.read_ttl_remaining(key) {
                Some(remaining) => format!("{}s", remaining.as_secs()),
                None => "none (persistent)".to_string(),
            };
            format!(
                "key     {key_str}\nsize    {}B\nttl     {ttl_info}\ntier    transient\n",
                value.len()
            )
        }
        None => format!("key {key_str} not found\n"),
    }
}

fn format_dump(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let flag = parts.next();
    let path = parts.next();
    let Some(("-f", path)) = flag.zip(path) else {
        return "usage: dump -f <path>\n".to_string();
    };
    let pairs = data_bus.find_by_prefix(b"");
    let mut file = match File::create(path) {
        Ok(file) => file,
        Err(error) => return format!("error: cannot create {path}: {error}\n"),
    };
    let mut count = 0usize;
    let mut bytes_written = 0usize;
    for (key, value) in &pairs {
        let key_len = (key.len() as u32).to_be_bytes();
        let value_len = (value.len() as u32).to_be_bytes();
        let ttl_ms = data_bus
            .read_ttl_remaining(key)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        let ttl_bytes = ttl_ms.to_be_bytes();
        let _ = file.write_all(&key_len);
        let _ = file.write_all(key);
        let _ = file.write_all(&value_len);
        let _ = file.write_all(value);
        let _ = file.write_all(&ttl_bytes);
        count += 1;
        bytes_written += 4 + key.len() + 4 + value.len() + 8;
    }
    format!("{count} entries  {bytes_written}B  written to {path}\n")
}

fn format_restore(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let flag = parts.next();
    let path = parts.next();
    let Some(("-f", path)) = flag.zip(path) else {
        return "usage: restore -f <path>\n".to_string();
    };
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return format!("error: cannot open {path}: {error}\n"),
    };
    let mut data = Vec::new();
    if file.read_to_end(&mut data).is_err() {
        return format!("error: cannot read {path}\n");
    }
    let mut offset = 0;
    let mut count = 0usize;
    while offset + 4 <= data.len() {
        let key_len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + key_len + 4 > data.len() {
            break;
        }
        let key = &data[offset..offset + key_len];
        offset += key_len;
        let value_len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + value_len + 8 > data.len() {
            break;
        }
        let value = &data[offset..offset + value_len];
        offset += value_len;
        let ttl_ms = u64::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;
        data_bus.write_value(key, value);
        if ttl_ms > 0 {
            data_bus.write_ttl(key, std::time::Duration::from_millis(ttl_ms));
        }
        count += 1;
    }
    format!("{count} entries restored from {path}\n")
}

fn format_shutdown() -> String {
    crate::modules::logger::log_info("shutdown requested via admin socket");
    std::process::exit(0);
}

fn format_help() -> String {
    "commands:\n  status              server status\n  keys [-p prefix]    list keys\n  inspect <key>       key metadata\n  dump -f <path>      export store to file\n  restore -f <path>   import store from file\n  reload              reload authorized_keys\n  shutdown            stop server\n  help                this message\n"
        .to_string()
}
