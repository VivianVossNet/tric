// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: tric — unified binary. `tric server` starts the daemon. Everything else is CLI.

use std::io::{self, BufRead, Write};
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        print_usage();
        std::process::exit(1);
    }

    if args[0] == "server" {
        run_server();
    } else if args[0] == "shell" {
        run_shell();
    } else {
        let command = args.join(" ");
        let response = send_command(&command);
        print!("{response}");
    }
}

fn run_server() {
    use tric::core::create_core;
    use tric::core::data_bus::DataBus;
    use tric::core::permutive_bus::create_permutive_bus;
    use tric::modules::cli::{create_cli, CliConfig};
    use tric::modules::metrics::create_metrics;
    use tric::modules::server::{create_server, ServerConfig};

    let socket_dir =
        std::env::var("TRIC_SOCKET_DIR").unwrap_or_else(|_| "/var/run/tric".to_string());
    let udp_bind = std::env::var("TRIC_UDP_BIND").unwrap_or_else(|_| "0.0.0.0:7483".to_string());
    let sqlite_dir =
        std::env::var("TRIC_SQLITE_DIR").unwrap_or_else(|_| "/var/db/tric".to_string());

    if let Err(error) = std::fs::create_dir_all(&socket_dir) {
        eprintln!("failed to create socket directory {socket_dir}: {error}");
        std::process::exit(1);
    }

    let local_path = format!("{socket_dir}/server.sock");
    let admin_path = format!("{socket_dir}/admin.sock");

    let data_bus: Arc<dyn DataBus> =
        Arc::new(create_permutive_bus(std::path::Path::new(&sqlite_dir)));
    let metrics = Arc::new(create_metrics());
    let mut core = create_core(data_bus);

    let metrics_for_server = Arc::clone(&metrics);
    let udp_bind_clone = udp_bind.clone();
    let local_path_clone = local_path.clone();
    core.register_module(move || {
        Box::new(create_server(
            ServerConfig {
                local_path: local_path_clone.clone(),
                udp_bind: udp_bind_clone.clone(),
                max_sessions: 10000,
            },
            Arc::clone(&metrics_for_server),
        ))
    });

    let metrics_for_cli = Arc::clone(&metrics);
    let admin_path_clone = admin_path.clone();
    core.register_module(move || {
        Box::new(create_cli(
            CliConfig {
                admin_path: admin_path_clone.clone(),
                auth_keys_path: None,
            },
            Arc::clone(&metrics_for_cli),
        ))
    });

    tric::modules::logger::log_info(&format!(
        "startup; local={local_path} udp={udp_bind} admin={admin_path}"
    ));

    core.run_supervision_loop();
}

fn run_shell() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        let _ = write!(stdout, "tric> ");
        let _ = stdout.flush();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == "exit" || trimmed == "quit" {
                    break;
                }
                let response = send_command(trimmed);
                print!("{response}");
            }
            Err(_) => break,
        }
    }
}

fn send_command(command: &str) -> String {
    let socket_dir =
        std::env::var("TRIC_SOCKET_DIR").unwrap_or_else(|_| "/var/run/tric".to_string());
    let admin_path = format!("{socket_dir}/admin.sock");
    let client_path = format!("/tmp/tric-cli-{}.sock", std::process::id());
    let _ = std::fs::remove_file(&client_path);

    let client = match UnixDatagram::bind(&client_path) {
        Ok(socket) => socket,
        Err(error) => {
            return format!("error: failed to bind {client_path}: {error}\n");
        }
    };

    if client.connect(&admin_path).is_err() {
        let _ = std::fs::remove_file(&client_path);
        return format!("error: cannot connect to {admin_path}\n");
    }

    if client.send(command.as_bytes()).is_err() {
        let _ = std::fs::remove_file(&client_path);
        return "error: failed to send command\n".to_string();
    }

    let mut buffer = [0u8; 65536];
    let result = match client.recv(&mut buffer) {
        Ok(length) => String::from_utf8_lossy(&buffer[..length]).to_string(),
        Err(error) => format!("error: failed to receive response: {error}\n"),
    };

    let _ = std::fs::remove_file(&client_path);
    result
}

fn print_usage() {
    eprintln!("usage: tric <command> [args...]");
    eprintln!("       tric server            start the daemon");
    eprintln!("       tric status            server status");
    eprintln!("       tric keys [-p prefix]  list keys");
    eprintln!("       tric inspect <key>     key metadata");
    eprintln!("       tric import -f <path> --format mysql|postgres|sqlite");
    eprintln!("       tric export -f <path.tric> [--debug] [--format mysql|postgres|sqlite]");
    eprintln!("       tric dump -f <path>    binary store dump");
    eprintln!("       tric restore -f <path> binary store restore");
    eprintln!("       tric shutdown          stop server");
    eprintln!("       tric shell             interactive REPL");
    eprintln!("       tric help              command list");
}
