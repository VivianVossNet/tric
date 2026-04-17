// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: CLI module — admin socket listener, text command parser, FreeBSD-style responses.

use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::time::Duration;

use crate::core::data_bus::DataBus;
use crate::core::module::{Module, ModuleContext};
use crate::modules::metrics::Metrics;

pub struct CliConfig {
    pub admin_path: String,
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

            let mut buffer = [0u8; 1024];
            let (length, peer) = match socket.recv_from(&mut buffer) {
                Ok(result) => result,
                Err(_) => continue,
            };

            let command = String::from_utf8_lossy(&buffer[..length]);
            let response = dispatch_command(command.trim(), &context.data_bus, &self.metrics);
            let _ = socket.send_to_addr(response.as_bytes(), &peer);
        }
    }
}

fn dispatch_command(command: &str, data_bus: &Arc<dyn DataBus>, metrics: &Metrics) -> String {
    let mut parts = command.split_whitespace();
    let verb = parts.next().unwrap_or("");

    match verb {
        "status" => format_status(metrics),
        "keys" => format_keys(parts, data_bus),
        "shutdown" => {
            crate::modules::logger::log_info("shutdown requested via admin socket");
            std::process::exit(0);
        }
        "help" => format_help(),
        _ => format!("error: unknown command '{verb}'\n"),
    }
}

fn format_status(metrics: &Metrics) -> String {
    format!(
        "tric-server\n  requests  {} total {} local {} network\n  errors    {}\n  sessions  {}\n  latency   {}us avg {}us max\n",
        metrics.read_requests_total(),
        metrics.read_requests_local(),
        metrics.read_requests_network(),
        metrics.read_errors_total(),
        metrics.read_active_sessions(),
        metrics.read_latency_average_microseconds(),
        metrics.read_latency_max_microseconds(),
    )
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

fn format_help() -> String {
    "commands:\n  status              server status\n  keys [-p prefix]    list keys\n  shutdown            graceful shutdown\n  help                this message\n".to_string()
}
