// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: TRIC+ Zig client — Connection struct wrapping the C bridge via @cImport.

const std = @import("std");

const c = @cImport({
    @cInclude("tric.h");
});

pub const Error = error{
    ConnectionInvalid,
    Communication,
    OutOfMemory,
};

pub const Pair = struct {
    key: []u8,
    value: []u8,
};

pub const Connection = struct {
    allocator: std.mem.Allocator,
    handle: c.TricConnection,
    owned: bool,

    pub fn init(allocator: std.mem.Allocator, socket_path: [:0]const u8) Connection {
        const handle = c.create_connection(socket_path.ptr);
        return .{
            .allocator = allocator,
            .handle = handle,
            .owned = handle.socket_fd >= 0,
        };
    }

    pub fn deinit(self: *Connection) void {
        if (self.owned) {
            c.delete_connection(&self.handle);
            self.owned = false;
        }
    }

    pub fn isValid(self: *Connection) bool {
        return self.owned and c.check_connection(&self.handle) != 0;
    }

    pub fn read(self: *Connection, key: []const u8) Error!?[]u8 {
        var value = c.read_value(&self.handle, key.ptr, key.len);
        if (value.data == null) return null;
        defer c.delete_value_result(&value);
        const copy = self.allocator.alloc(u8, value.length) catch return Error.OutOfMemory;
        @memcpy(copy, value.data[0..value.length]);
        return copy;
    }

    pub fn write(self: *Connection, key: []const u8, value: []const u8) Error!void {
        const result = c.write_value(&self.handle, key.ptr, key.len, value.ptr, value.len);
        if (result != 0) return Error.Communication;
    }

    pub fn del(self: *Connection, key: []const u8) Error!void {
        const result = c.delete_value(&self.handle, key.ptr, key.len);
        if (result != 0) return Error.Communication;
    }

    pub fn cad(self: *Connection, key: []const u8, expected: []const u8) Error!bool {
        const result = c.delete_value_if_match(&self.handle, key.ptr, key.len, expected.ptr, expected.len);
        if (result < 0) return Error.Communication;
        return result == 1;
    }

    pub fn ttl(self: *Connection, key: []const u8, duration_ms: u64) Error!void {
        const result = c.write_ttl(&self.handle, key.ptr, key.len, duration_ms);
        if (result != 0) return Error.Communication;
    }

    pub fn scan(self: *Connection, prefix: []const u8) Error![]Pair {
        var result = c.find_by_prefix(&self.handle, prefix.ptr, prefix.len);
        defer c.delete_scan_result(&result);
        const pairs = self.allocator.alloc(Pair, result.count) catch return Error.OutOfMemory;
        errdefer self.allocator.free(pairs);
        var filled: usize = 0;
        errdefer for (pairs[0..filled]) |p| {
            self.allocator.free(p.key);
            self.allocator.free(p.value);
        };
        while (filled < result.count) : (filled += 1) {
            const src = result.pairs[filled];
            const key_copy = self.allocator.alloc(u8, src.key_length) catch return Error.OutOfMemory;
            @memcpy(key_copy, src.key[0..src.key_length]);
            const value_copy = self.allocator.alloc(u8, src.value_length) catch {
                self.allocator.free(key_copy);
                return Error.OutOfMemory;
            };
            @memcpy(value_copy, src.value[0..src.value_length]);
            pairs[filled] = .{ .key = key_copy, .value = value_copy };
        }
        return pairs;
    }
};

pub fn freeScan(allocator: std.mem.Allocator, pairs: []Pair) void {
    for (pairs) |p| {
        allocator.free(p.key);
        allocator.free(p.value);
    }
    allocator.free(pairs);
}
