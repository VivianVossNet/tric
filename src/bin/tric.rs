// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: tric CLI binary — connects to admin socket, sends command, displays response.

use std::os::unix::net::UnixDatagram;

const DEFAULT_ADMIN_PATH: &str = "/var/run/tric/admin.sock";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: tric <command> [args...]");
        eprintln!("       tric status");
        eprintln!("       tric keys [-p prefix]");
        eprintln!("       tric shutdown");
        eprintln!("       tric help");
        std::process::exit(1);
    }

    let command = args.join(" ");
    let admin_path = std::env::var("TRIC_ADMIN_SOCKET").unwrap_or(DEFAULT_ADMIN_PATH.to_string());

    let client_path = format!("/tmp/tric-cli-{}.sock", std::process::id());
    let _ = std::fs::remove_file(&client_path);
    let client = UnixDatagram::bind(&client_path).unwrap_or_else(|error| {
        eprintln!("failed to bind client socket {client_path}: {error}");
        std::process::exit(1);
    });

    client.connect(&admin_path).unwrap_or_else(|error| {
        eprintln!("failed to connect to {admin_path}: {error}");
        let _ = std::fs::remove_file(&client_path);
        std::process::exit(1);
    });

    client.send(command.as_bytes()).unwrap_or_else(|error| {
        eprintln!("failed to send command: {error}");
        let _ = std::fs::remove_file(&client_path);
        std::process::exit(1);
    });

    let mut buffer = [0u8; 65536];
    match client.recv(&mut buffer) {
        Ok(length) => {
            print!("{}", String::from_utf8_lossy(&buffer[..length]));
        }
        Err(error) => {
            eprintln!("failed to receive response: {error}");
        }
    }

    let _ = std::fs::remove_file(&client_path);
}
