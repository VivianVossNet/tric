// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: Integration test for the TRIC+ C++ bridge — verifies all six primitives against a running server.

#include "tric.hpp"

#include <cstdio>
#include <cstdlib>
#include <string>
#include <utility>

static int checks_passed = 0;
static int checks_failed = 0;

static void check(const char* label, bool condition) {
    if (condition) {
        ++checks_passed;
    } else {
        ++checks_failed;
        std::fprintf(stderr, "FAIL: %s\n", label);
    }
}

int main(int argc, char* argv[]) {
    const char* socket_path = argc > 1 ? argv[1] : "/var/run/tric/server.sock";

    tric::connection c(socket_path);
    check("connection valid", c.valid());
    if (!c) {
        std::fprintf(stderr, "cannot connect to %s\n", socket_path);
        return 1;
    }

    c.write("test:1", "hello");
    auto v = c.read("test:1");
    check("read returns value", v.has_value());
    check("read correct length", v && v->size() == 5);
    check("read correct content", v && *v == "hello");

    c.write("test:1", "updated");
    v = c.read("test:1");
    check("write overwrites", v && *v == "updated");

    c.del("test:1");
    v = c.read("test:1");
    check("del removes key", !v.has_value());

    c.write("test:cas", "original");
    bool matched = c.cad("test:cas", "wrong");
    check("cad mismatch returns false", !matched);
    v = c.read("test:cas");
    check("cad mismatch keeps value", v.has_value());

    matched = c.cad("test:cas", "original");
    check("cad match returns true", matched);
    v = c.read("test:cas");
    check("cad match deletes", !v.has_value());

    c.write("test:ttl", "ephemeral");
    bool ttl_ok = c.ttl("test:ttl", 60000);
    check("ttl succeeds", ttl_ok);
    v = c.read("test:ttl");
    check("ttl key still readable", v.has_value());
    c.del("test:ttl");

    c.write("scan:a", "1");
    c.write("scan:b", "2");
    c.write("scan:c", "3");
    auto pairs = c.scan("scan:");
    check("scan returns results", pairs.size() >= 3);
    c.del("scan:a");
    c.del("scan:b");
    c.del("scan:c");

    tric::connection moved(std::move(c));
    check("move construction transfers ownership", moved.valid());

    std::printf("%d passed, %d failed\n", checks_passed, checks_failed);
    return checks_failed > 0 ? 1 : 0;
}
