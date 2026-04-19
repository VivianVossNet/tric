// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: Integration test for the TRIC+ Zig bridge — verifies all six primitives against a running server.

const std = @import("std");
const tric = @import("tric");

const default_socket_path: [:0]const u8 = "/tmp/tric-zig-test/server.sock";

fn resolveSocketPath() [:0]const u8 {
    if (std.c.getenv("TRIC_SOCKET")) |raw| {
        return std.mem.span(raw);
    }
    return default_socket_path;
}

fn openConnection(allocator: std.mem.Allocator) tric.Connection {
    return tric.Connection.init(allocator, resolveSocketPath());
}

test "connection is valid" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try std.testing.expect(connection.isValid());
}

test "read returns written value" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:1", "hello");
    const value = try connection.read("test:1");
    try std.testing.expect(value != null);
    if (value) |v| allocator.free(v);
}

test "read returns correct length" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:len", "hello");
    const value = (try connection.read("test:len")) orelse return error.TestUnexpectedResult;
    defer allocator.free(value);
    try std.testing.expectEqual(@as(usize, 5), value.len);
}

test "read returns correct content" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:content", "hello");
    const value = (try connection.read("test:content")) orelse return error.TestUnexpectedResult;
    defer allocator.free(value);
    try std.testing.expectEqualStrings("hello", value);
}

test "write overwrites" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:over", "original");
    try connection.write("test:over", "updated");
    const value = (try connection.read("test:over")) orelse return error.TestUnexpectedResult;
    defer allocator.free(value);
    try std.testing.expectEqualStrings("updated", value);
}

test "del removes key" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:del", "payload");
    try connection.del("test:del");
    const value = try connection.read("test:del");
    try std.testing.expect(value == null);
}

test "cad mismatch returns false" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:cas-miss", "original");
    const matched = try connection.cad("test:cas-miss", "wrong");
    try std.testing.expect(!matched);
}

test "cad mismatch keeps value" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:cas-keep", "original");
    _ = try connection.cad("test:cas-keep", "wrong");
    const value = try connection.read("test:cas-keep");
    try std.testing.expect(value != null);
    if (value) |v| allocator.free(v);
}

test "cad match returns true" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:cas-match", "original");
    const matched = try connection.cad("test:cas-match", "original");
    try std.testing.expect(matched);
}

test "cad match deletes" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:cas-del", "original");
    _ = try connection.cad("test:cas-del", "original");
    const value = try connection.read("test:cas-del");
    try std.testing.expect(value == null);
}

test "ttl succeeds" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:ttl", "ephemeral");
    try connection.ttl("test:ttl", 60_000);
}

test "ttl key still readable" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("test:ttl-read", "ephemeral");
    try connection.ttl("test:ttl-read", 60_000);
    const value = try connection.read("test:ttl-read");
    try std.testing.expect(value != null);
    if (value) |v| allocator.free(v);
}

test "scan returns results" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    try connection.write("scan:a", "1");
    try connection.write("scan:b", "2");
    try connection.write("scan:c", "3");
    const pairs = try connection.scan("scan:");
    defer tric.freeScan(allocator, pairs);
    try std.testing.expect(pairs.len >= 3);
    try connection.del("scan:a");
    try connection.del("scan:b");
    try connection.del("scan:c");
}

test "slice inputs of varying lengths round-trip" {
    const allocator = std.testing.allocator;
    var connection = openConnection(allocator);
    defer connection.deinit();
    const value = "value with spaces and more bytes";
    try connection.write("test:slice", value);
    const roundtrip = (try connection.read("test:slice")) orelse return error.TestUnexpectedResult;
    defer allocator.free(roundtrip);
    try std.testing.expectEqualStrings(value, roundtrip);
}
