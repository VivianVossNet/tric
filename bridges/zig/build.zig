// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: TRIC+ Zig client — build graph wiring the C bridge + Zig wrapper + integration test.

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const tric_module = b.addModule("tric", .{
        .root_source_file = b.path("src/root.zig"),
        .target = target,
    });
    tric_module.addIncludePath(b.path("../c"));
    tric_module.addCSourceFile(.{
        .file = b.path("../c/tric.c"),
        .flags = &.{ "-std=c11", "-Wall", "-Wextra", "-Wpedantic" },
    });

    const test_module = b.createModule(.{
        .root_source_file = b.path("tests/bridge_test.zig"),
        .target = target,
        .optimize = optimize,
    });
    test_module.addImport("tric", tric_module);

    const tests = b.addTest(.{
        .root_module = test_module,
    });

    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run TRIC+ Zig bridge integration tests");
    test_step.dependOn(&run_tests.step);
}
