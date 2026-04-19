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
        let socket = match UnixDatagram::bind(&self.config.admin_path) {
            Ok(socket) => socket,
            Err(error) => {
                eprintln!(
                    "tric: cannot bind admin socket {}: {error}",
                    self.config.admin_path
                );
                std::process::exit(1);
            }
        };

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
            "status" => self.render_status(),
            "keys" => render_keys(parts, data_bus),
            "inspect" => render_inspect(parts, data_bus),
            "import" => render_import(parts, data_bus),
            "query" => render_query(parts, data_bus),
            "export" => render_export(parts, data_bus),
            "dump" => render_dump(parts, data_bus),
            "restore" => render_restore(parts, data_bus),
            "reload" => self.render_reload(),
            "shutdown" => render_shutdown(),
            "help" => render_help(),
            _ => format!("error: unknown command '{verb}'\n"),
        }
    }

    fn render_status(&self) -> String {
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

    fn render_reload(&self) -> String {
        match &self.config.auth_keys_path {
            Some(path) => {
                crate::modules::logger::log_info("reload; source=authorized_keys trigger=admin");
                format!("reloaded {path}\n")
            }
            None => "auth disabled (--no-auth); nothing to reload\n".to_string(),
        }
    }
}

fn render_keys(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
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

fn render_inspect(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
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

fn render_dump(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
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

fn render_restore(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
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

fn render_import(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let flag_f = parts.next();

    if flag_f == Some("-D") || flag_f == Some("--diff") {
        let old_path = parts.next();
        let new_path = parts.next();
        let Some((old_path, new_path)) = old_path.zip(new_path) else {
            return "usage: import -D <old.tric> <new.tric>\n".to_string();
        };
        return render_diff_import(old_path, new_path, data_bus);
    }

    let path = parts.next();
    let flag_format = parts.next();
    let format = parts.next();
    let analyse_only = parts.next() == Some("-a") || parts.next() == Some("--analyse");

    let Some(("-f", path)) = flag_f.zip(path) else {
        return "usage: import -f <path> -F mysql|postgres|sqlite [-a]\n       import -D <old.tric> <new.tric>\n"
            .to_string();
    };
    let Some(("-F" | "--format", format)) = flag_format.zip(format) else {
        return "usage: import -f <path> -F mysql|postgres|sqlite [-a]\n       import -D <old.tric> <new.tric>\n"
            .to_string();
    };

    let max_file_size: u64 = 1_073_741_824;
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.len() > max_file_size => {
            return format!("error: file exceeds 1 GB limit ({}B)\n", metadata.len());
        }
        Err(error) => return format!("error: cannot stat {path}: {error}\n"),
        _ => {}
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => return format!("error: cannot read {path}: {error}\n"),
    };

    let statements = crate::modules::import::parse_sql(&content, format);
    let plan = crate::modules::analyser::analyse_statements(&statements);

    if analyse_only {
        return crate::modules::analyser::render_storage_plan(&plan);
    }

    let result = crate::modules::import::parse_import(&statements, &plan, data_bus);
    format!(
        "{} tables, {} rows, {} relationships imported. {} errors.\n",
        result.tables, result.rows, result.relationships, result.errors
    )
}

fn render_diff_import(old_path: &str, new_path: &str, data_bus: &Arc<dyn DataBus>) -> String {
    let max_file_size: u64 = 1_073_741_824;
    for path in [old_path, new_path] {
        match std::fs::metadata(path) {
            Ok(metadata) if metadata.len() > max_file_size => {
                return format!("error: {path} exceeds 1 GB limit ({}B)\n", metadata.len());
            }
            Err(error) => return format!("error: cannot stat {path}: {error}\n"),
            _ => {}
        }
    }

    match crate::modules::import::parse_diff_import(old_path, new_path, data_bus) {
        Ok(result) => format!(
            "{} additions, {} modifications, {} deletions applied.\n",
            result.additions, result.modifications, result.deletions
        ),
        Err(error) => format!("error: {error}\n"),
    }
}

fn render_query(parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let sql: String = parts.collect::<Vec<&str>>().join(" ");
    if sql.is_empty() {
        return "usage: query <SQL statement>\n".to_string();
    }
    if sql.len() > 4096 {
        return "error: SQL exceeds 4096 byte limit\n".to_string();
    }
    let responses = crate::modules::query::parse_query(&sql, 0, data_bus);
    let mut output = String::new();
    for response in &responses {
        match response.opcode {
            0x80 => output.push_str("OK\n"),
            0x81 if response.payload.len() >= 4 => {
                let value_len = u32::from_be_bytes([
                    response.payload[0],
                    response.payload[1],
                    response.payload[2],
                    response.payload[3],
                ]) as usize;
                if response.payload.len() >= 4 + value_len {
                    let value = &response.payload[4..4 + value_len];
                    output.push_str(&String::from_utf8_lossy(value));
                    output.push('\n');
                }
            }
            0x90 if response.payload.len() >= 4 => {
                let key_start = 4;
                if let Some(key_data) = parse_scan_key(&response.payload[key_start..]) {
                    output.push_str(&format!("{}  ", String::from_utf8_lossy(key_data)));
                }
                if let Some(value_data) = parse_scan_value(&response.payload[key_start..]) {
                    output.push_str(&format!("{}B", value_data.len()));
                }
                output.push('\n');
            }
            0x91 => {}
            0xA1 => {
                let msg = String::from_utf8_lossy(&response.payload);
                output.push_str(&format!("error: {msg}\n"));
            }
            _ => {}
        }
    }
    if output.is_empty() {
        "(no results)\n".to_string()
    } else {
        output
    }
}

fn parse_scan_key(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() >= 4 + len {
        Some(&data[4..4 + len])
    } else {
        None
    }
}

fn parse_scan_value(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 4 {
        return None;
    }
    let key_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let value_offset = 4 + key_len;
    if data.len() < value_offset + 4 {
        return None;
    }
    let value_len = u32::from_be_bytes([
        data[value_offset],
        data[value_offset + 1],
        data[value_offset + 2],
        data[value_offset + 3],
    ]) as usize;
    let value_start = value_offset + 4;
    if data.len() >= value_start + value_len {
        Some(&data[value_start..value_start + value_len])
    } else {
        None
    }
}

fn render_export(mut parts: std::str::SplitWhitespace, data_bus: &Arc<dyn DataBus>) -> String {
    let flag_f = parts.next();
    let path = parts.next();
    let Some(("-f", path)) = flag_f.zip(path) else {
        return "usage: export -f <path.tric> [-d] [-F mysql|postgres|sqlite]\n".to_string();
    };

    let mut debug = false;
    let mut sql_format: Option<&str> = None;

    while let Some(arg) = parts.next() {
        match arg {
            "-d" | "--debug" => debug = true,
            "-F" | "--format" => sql_format = parts.next(),
            _ => {}
        }
    }

    if let Some(dialect) = sql_format {
        match crate::modules::export::write_sql_file(data_bus, path, dialect) {
            Ok(result) => format!("{} rows exported to {path} ({})\n", result.entries, dialect),
            Err(error) => format!("error: {error}\n"),
        }
    } else {
        match crate::modules::export::write_tric_archive(data_bus, path, debug) {
            Ok(result) => {
                let mode = if debug { "uncompressed tar" } else { "Brotli" };
                format!("{} entries exported to {path} ({mode})\n", result.entries)
            }
            Err(error) => format!("error: {error}\n"),
        }
    }
}

fn render_shutdown() -> String {
    crate::modules::logger::log_info("shutdown requested via admin socket");
    std::process::exit(0);
}

fn render_help() -> String {
    "commands:\n  status                                server status\n  keys [-p prefix]                      list keys\n  inspect <key>                         key metadata\n  query <SQL>                           SQL query\n  import -f <path> -F mysql|postgres|sqlite [-a]\n  import -D <old.tric> <new.tric>       diff-import\n  export -f <path.tric> [-d] [-F mysql|postgres|sqlite]\n  dump -f <path>                        binary store dump\n  restore -f <path>                     binary store restore\n  reload                                reload authorized_keys\n  shutdown                              stop server\n  help                                  this message\n".to_string()
}
