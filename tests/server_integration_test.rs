// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: End-to-end server integration test — starts tric-server, tests all functions via UDS + CLI.

use std::os::unix::net::UnixDatagram;
use std::process::{Child, Command};
use std::time::Duration;

const SOCKET_DIR: &str = "/tmp/tric-integration-test";
const SERVER_SOCK: &str = "/tmp/tric-integration-test/server.sock";
const ADMIN_SOCK: &str = "/tmp/tric-integration-test/admin.sock";

#[allow(clippy::zombie_processes)]
fn create_server() -> Child {
    let _ = std::fs::remove_dir_all(SOCKET_DIR);
    std::fs::create_dir_all(SOCKET_DIR).unwrap();

    let child = Command::new("target/release/tric")
        .arg("server")
        .env("TRIC_SOCKET_DIR", SOCKET_DIR)
        .spawn()
        .expect("failed to start tric server");

    for _ in 0..50 {
        if std::path::Path::new(SERVER_SOCK).exists() && std::path::Path::new(ADMIN_SOCK).exists() {
            return child;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("tric-server did not create sockets within 5 seconds");
}

fn send_datagram(data: &[u8]) -> Vec<u8> {
    let client_path = format!("{SOCKET_DIR}/test-client-{}.sock", std::process::id());
    let _ = std::fs::remove_file(&client_path);
    let client = UnixDatagram::bind(&client_path).unwrap();
    client.connect(SERVER_SOCK).unwrap();
    client.send(data).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buffer = [0u8; 65536];
    let length = client.recv(&mut buffer).unwrap();
    let _ = std::fs::remove_file(&client_path);
    buffer[..length].to_vec()
}

fn send_admin(command: &str) -> String {
    let client_path = format!("{SOCKET_DIR}/test-admin-{}.sock", std::process::id());
    let _ = std::fs::remove_file(&client_path);
    let client = UnixDatagram::bind(&client_path).unwrap();
    client.connect(ADMIN_SOCK).unwrap();
    client.send(command.as_bytes()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buffer = [0u8; 65536];
    let length = client.recv(&mut buffer).unwrap();
    let _ = std::fs::remove_file(&client_path);
    String::from_utf8_lossy(&buffer[..length]).to_string()
}

fn build_write_value(request_id: u32, key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut datagram = Vec::new();
    datagram.extend_from_slice(&request_id.to_be_bytes());
    datagram.push(0x02);
    datagram.extend_from_slice(&(key.len() as u32).to_be_bytes());
    datagram.extend_from_slice(key);
    datagram.extend_from_slice(&(value.len() as u32).to_be_bytes());
    datagram.extend_from_slice(value);
    datagram
}

fn build_read_value(request_id: u32, key: &[u8]) -> Vec<u8> {
    let mut datagram = Vec::new();
    datagram.extend_from_slice(&request_id.to_be_bytes());
    datagram.push(0x01);
    datagram.extend_from_slice(&(key.len() as u32).to_be_bytes());
    datagram.extend_from_slice(key);
    datagram
}

fn build_delete_value(request_id: u32, key: &[u8]) -> Vec<u8> {
    let mut datagram = Vec::new();
    datagram.extend_from_slice(&request_id.to_be_bytes());
    datagram.push(0x03);
    datagram.extend_from_slice(&(key.len() as u32).to_be_bytes());
    datagram.extend_from_slice(key);
    datagram
}

fn build_find_by_prefix(request_id: u32, prefix: &[u8]) -> Vec<u8> {
    let mut datagram = Vec::new();
    datagram.extend_from_slice(&request_id.to_be_bytes());
    datagram.push(0x06);
    datagram.extend_from_slice(&(prefix.len() as u32).to_be_bytes());
    datagram.extend_from_slice(prefix);
    datagram
}

fn build_query(request_id: u32, sql: &[u8]) -> Vec<u8> {
    let mut datagram = Vec::new();
    datagram.extend_from_slice(&request_id.to_be_bytes());
    datagram.push(0x07);
    datagram.extend_from_slice(&(sql.len() as u32).to_be_bytes());
    datagram.extend_from_slice(sql);
    datagram
}

fn build_ping(request_id: u32) -> Vec<u8> {
    let mut datagram = Vec::new();
    datagram.extend_from_slice(&request_id.to_be_bytes());
    datagram.push(0x13);
    datagram
}

fn check_response_opcode(response: &[u8], expected_opcode: u8) {
    assert!(
        response.len() >= 5,
        "response too short: {} bytes",
        response.len()
    );
    assert_eq!(
        response[4], expected_opcode,
        "expected opcode 0x{expected_opcode:02x}, got 0x{:02x}",
        response[4]
    );
}

#[test]
fn check_full_server_integration() {
    let server = create_server();

    struct ServerGuard(Child);
    impl Drop for ServerGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
            let _ = std::fs::remove_dir_all(SOCKET_DIR);
        }
    }
    let _guard = ServerGuard(server);

    check_ping_response();
    check_write_and_read_value();
    check_delete_value();
    check_find_by_prefix();
    check_admin_status();
    check_admin_keys();
    check_admin_inspect();
    check_admin_help();
    check_admin_dump_and_restore();
    check_sql_import();
    check_export_tric();
    check_export_tric_debug();
    check_export_sql();
    check_export_roundtrip();
    check_query_insert_and_select();
    check_query_update_and_delete();
    check_query_select_all();
    check_query_select_like_prefix();
    check_query_wire_protocol();
    check_query_error_handling();

    // ServerGuard handles cleanup via Drop
}

fn check_ping_response() {
    let response = send_datagram(&build_ping(1));
    check_response_opcode(&response, 0x80);
}

fn check_write_and_read_value() {
    let response = send_datagram(&build_write_value(10, b"user:42", b"Alice"));
    check_response_opcode(&response, 0x80);

    let response = send_datagram(&build_read_value(11, b"user:42"));
    check_response_opcode(&response, 0x81);
    let value_len =
        u32::from_be_bytes([response[5], response[6], response[7], response[8]]) as usize;
    let value = &response[9..9 + value_len];
    assert_eq!(value, b"Alice");
}

fn check_delete_value() {
    send_datagram(&build_write_value(20, b"temp:key", b"temporary"));
    let response = send_datagram(&build_read_value(21, b"temp:key"));
    check_response_opcode(&response, 0x81);

    send_datagram(&build_delete_value(22, b"temp:key"));
    let response = send_datagram(&build_read_value(23, b"temp:key"));
    check_response_opcode(&response, 0x80);
}

fn check_find_by_prefix() {
    send_datagram(&build_write_value(30, b"product:1", b"Widget"));
    send_datagram(&build_write_value(31, b"product:2", b"Gadget"));
    send_datagram(&build_write_value(32, b"other:x", b"ignored"));

    let response = send_datagram(&build_find_by_prefix(33, b"product:"));
    check_response_opcode(&response, 0x90);
}

fn check_admin_status() {
    let response = send_admin("status");
    assert!(
        response.contains("tric-server"),
        "status should contain 'tric-server'"
    );
    assert!(
        response.contains("requests"),
        "status should contain 'requests'"
    );
}

fn check_admin_keys() {
    let response = send_admin("keys -p user:");
    assert!(response.contains("user:42"), "keys should list user:42");
}

fn check_admin_inspect() {
    let response = send_admin("inspect user:42");
    assert!(response.contains("key"), "inspect should show key info");
    assert!(response.contains("size"), "inspect should show size");
}

fn check_admin_help() {
    let response = send_admin("help");
    assert!(response.contains("status"), "help should list status");
    assert!(response.contains("import"), "help should list import");
    assert!(response.contains("shutdown"), "help should list shutdown");
}

fn check_admin_dump_and_restore() {
    let dump_path = format!("{SOCKET_DIR}/test-dump.bin");
    let response = send_admin(&format!("dump -f {dump_path}"));
    assert!(response.contains("written"), "dump should confirm write");

    send_datagram(&build_delete_value(40, b"user:42"));
    let response = send_datagram(&build_read_value(41, b"user:42"));
    check_response_opcode(&response, 0x80);

    let response = send_admin(&format!("restore -f {dump_path}"));
    assert!(response.contains("restored"), "restore should confirm");

    let response = send_datagram(&build_read_value(42, b"user:42"));
    check_response_opcode(&response, 0x81);
}

fn check_sql_import() {
    let sql_path = format!("{SOCKET_DIR}/test.sql");
    std::fs::write(
        &sql_path,
        "CREATE TABLE customers (id INT PRIMARY KEY, name VARCHAR(255));\n\
         INSERT INTO customers VALUES (1, 'TestUser');\n\
         INSERT INTO customers VALUES (2, 'AnotherUser');\n",
    )
    .unwrap();

    let response = send_admin(&format!("import -f {sql_path} --format sqlite"));
    assert!(response.contains("tables"), "import should report tables");
    assert!(response.contains("rows"), "import should report rows");

    let response = send_admin("keys -p customers:");
    assert!(
        response.contains("customers:1"),
        "imported key customers:1 should exist"
    );

    let response = send_admin("keys -p _schema:");
    assert!(
        response.contains("_schema:customers"),
        "schema entry should exist"
    );
}

fn check_export_tric() {
    let export_path = format!("{SOCKET_DIR}/test-export.tric");
    let response = send_admin(&format!("export -f {export_path}"));
    assert!(
        response.contains("exported"),
        "export should confirm: {response}"
    );
    assert!(
        std::path::Path::new(&export_path).exists(),
        ".tric file should exist"
    );
    let file_size = std::fs::metadata(&export_path).unwrap().len();
    assert!(file_size > 0, ".tric file should not be empty");
}

fn check_export_tric_debug() {
    let export_path = format!("{SOCKET_DIR}/test-export-debug.tric");
    let response = send_admin(&format!("export -f {export_path} --debug"));
    assert!(
        response.contains("exported"),
        "debug export should confirm: {response}"
    );
    assert!(
        response.contains("uncompressed"),
        "debug export should say uncompressed: {response}"
    );
}

fn check_export_sql() {
    let export_path = format!("{SOCKET_DIR}/test-export.sql");
    let response = send_admin(&format!("export -f {export_path} --format sqlite"));
    assert!(
        response.contains("exported"),
        "SQL export should confirm: {response}"
    );
    let content = std::fs::read_to_string(&export_path).unwrap();
    assert!(
        content.contains("CREATE TABLE"),
        "SQL export should contain CREATE TABLE"
    );
    assert!(
        content.contains("INSERT INTO"),
        "SQL export should contain INSERT INTO"
    );
}

fn check_export_roundtrip() {
    let export_path = format!("{SOCKET_DIR}/roundtrip.sql");
    send_admin(&format!("export -f {export_path} --format sqlite"));

    send_datagram(&build_delete_value(50, b"customers:1"));
    send_datagram(&build_delete_value(51, b"customers:2"));
    let response = send_datagram(&build_read_value(52, b"customers:1"));
    check_response_opcode(&response, 0x80);

    send_admin(&format!("import -f {export_path} --format sqlite"));
    let response = send_datagram(&build_read_value(53, b"customers:1"));
    check_response_opcode(&response, 0x81);
}

fn check_query_insert_and_select() {
    let response = send_admin("query INSERT INTO orders VALUES (100, 'Widget', 42)");
    assert_eq!(response.trim(), "OK", "INSERT via query should return OK");

    let response = send_admin("query SELECT * FROM orders WHERE key = '100'");
    assert!(
        response.contains("Widget"),
        "SELECT by key should return inserted value: {response}"
    );
}

fn check_query_update_and_delete() {
    send_admin("query INSERT INTO items VALUES (200, 'Original')");

    let response = send_admin("query UPDATE items SET value = 'Updated' WHERE key = '200'");
    assert_eq!(response.trim(), "OK", "UPDATE via query should return OK");

    let response = send_admin("query SELECT * FROM items WHERE key = '200'");
    assert!(
        response.contains("Updated"),
        "SELECT after UPDATE should return new value: {response}"
    );

    let response = send_admin("query DELETE FROM items WHERE key = '200'");
    assert_eq!(response.trim(), "OK", "DELETE via query should return OK");

    let response = send_admin("query SELECT * FROM items WHERE key = '200'");
    assert_eq!(
        response.trim(),
        "OK",
        "SELECT after DELETE should return empty OK: {response}"
    );
}

fn check_query_select_all() {
    send_admin("query INSERT INTO colours VALUES (1, 'red')");
    send_admin("query INSERT INTO colours VALUES (2, 'green')");
    send_admin("query INSERT INTO colours VALUES (3, 'blue')");

    let response = send_admin("query SELECT * FROM colours");
    assert!(
        response.contains("colours:1"),
        "SELECT all should list colours:1: {response}"
    );
    assert!(
        response.contains("colours:3"),
        "SELECT all should list colours:3: {response}"
    );
}

fn check_query_select_like_prefix() {
    send_admin("query INSERT INTO tags VALUES ('lang-rust', 'systems')");
    send_admin("query INSERT INTO tags VALUES ('lang-go', 'gc')");
    send_admin("query INSERT INTO tags VALUES ('db-sqlite', 'embedded')");

    let response = send_admin("query SELECT * FROM tags WHERE key LIKE 'lang-%'");
    assert!(
        response.contains("tags:lang-rust"),
        "LIKE prefix should match lang-rust: {response}"
    );
    assert!(
        response.contains("tags:lang-go"),
        "LIKE prefix should match lang-go: {response}"
    );
    assert!(
        !response.contains("tags:db-sqlite"),
        "LIKE prefix should not match db-sqlite: {response}"
    );
}

fn check_query_wire_protocol() {
    send_admin("query INSERT INTO wire VALUES (1, 'test-wire')");

    let response = send_datagram(&build_query(60, b"SELECT * FROM wire WHERE key = '1'"));
    check_response_opcode(&response, 0x81);
    let value_len =
        u32::from_be_bytes([response[5], response[6], response[7], response[8]]) as usize;
    let value = &response[9..9 + value_len];
    assert_eq!(
        std::str::from_utf8(value).unwrap(),
        "test-wire",
        "QUERY opcode 0x07 should return correct value"
    );
}

fn check_query_error_handling() {
    let response = send_admin("query THIS IS NOT VALID SQL !!!");
    assert!(
        response.contains("error"),
        "invalid SQL should return error: {response}"
    );

    let response = send_admin("query DROP TABLE users");
    assert!(
        response.contains("error"),
        "unsupported statement should return error: {response}"
    );
}
