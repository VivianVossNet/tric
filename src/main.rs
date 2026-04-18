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
    } else if args[0] == "slots" {
        run_slots();
    } else if args[0] == "clone" {
        run_clone(&args);
    } else if args[0] == "benchmark" || args[0] == "-b" {
        run_benchmark();
    } else if args[0] == "-h" {
        print_usage();
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
    let base_dir = std::env::var("TRIC_BASE_DIR").unwrap_or_else(|_| "/var/db/tric".to_string());
    let instance = std::env::var("TRIC_INSTANCE").unwrap_or_else(|_| "default".to_string());
    let slot: u32 = std::env::var("TRIC_SLOT")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .unwrap_or(0);

    if let Err(error) = std::fs::create_dir_all(&socket_dir) {
        eprintln!("failed to create socket directory {socket_dir}: {error}");
        std::process::exit(1);
    }

    let local_path = format!("{socket_dir}/server.sock");
    let admin_path = format!("{socket_dir}/admin.sock");

    let data_bus: Arc<dyn DataBus> = Arc::new(create_permutive_bus(
        std::path::Path::new(&base_dir),
        &instance,
        slot,
    ));
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

fn run_benchmark() {
    tric::modules::benchmark::run_benchmark();
}

fn run_slots() {
    let base_dir = std::env::var("TRIC_BASE_DIR").unwrap_or_else(|_| "/var/db/tric".to_string());
    let instance = std::env::var("TRIC_INSTANCE").unwrap_or_else(|_| "default".to_string());

    let slots =
        tric::core::sqlite_bus::find_instance_slots(std::path::Path::new(&base_dir), &instance);

    if slots.is_empty() {
        println!("no slots for instance '{instance}' in {base_dir}");
        return;
    }

    for (slot, size) in &slots {
        let label = if *slot == 0 { "  (primary)" } else { "" };
        println!("  {instance}_{slot}  {size}B{label}");
    }
}

fn run_clone(args: &[String]) {
    let base_dir = std::env::var("TRIC_BASE_DIR").unwrap_or_else(|_| "/var/db/tric".to_string());
    let instance = std::env::var("TRIC_INSTANCE").unwrap_or_else(|_| "default".to_string());
    let source_slot: u32 = std::env::var("TRIC_SLOT")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .unwrap_or(0);

    let Some(target_str) = args.get(1) else {
        eprintln!("usage: tric clone <target-slot>");
        std::process::exit(1);
    };
    let Ok(target_slot) = target_str.parse::<u32>() else {
        eprintln!("error: target slot must be a number");
        std::process::exit(1);
    };

    match tric::core::sqlite_bus::create_clone(
        std::path::Path::new(&base_dir),
        &instance,
        source_slot,
        target_slot,
    ) {
        Ok(bytes) => {
            println!("cloned {instance}_{source_slot} → {instance}_{target_slot}  ({bytes}B)")
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("usage: tric <command> [args...]");
    eprintln!("       tric server                          start daemon");
    eprintln!("       tric status                          server status");
    eprintln!("       tric keys [-p prefix]                list keys");
    eprintln!("       tric inspect <key>                   key metadata");
    eprintln!("       tric query <SQL>                     SQL query");
    eprintln!("       tric import -f <path> -F <dialect> [-a]");
    eprintln!("       tric import -D <old.tric> <new.tric>");
    eprintln!("       tric export -f <path> [-d] [-F <dialect>]");
    eprintln!("       tric dump -f <path>                  binary store dump");
    eprintln!("       tric restore -f <path>               binary store restore");
    eprintln!("       tric slots                           list instance slots");
    eprintln!("       tric clone <slot>                    clone current slot");
    eprintln!("       tric -b                              performance benchmark");
    eprintln!("       tric shutdown                        stop server");
    eprintln!("       tric shell                           interactive REPL");
    eprintln!("       tric -h                              help");
}
