/* Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License. */
/* SPDX-License-Identifier: BSD-3-Clause */
/* Scope: Integration test for the TRIC+ C bridge — verifies all six primitives against a running server. */

#include "tric.h"

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

static int checks_passed = 0;
static int checks_failed = 0;

static void check(const char *label, int condition) {
    if (condition) {
        checks_passed++;
    } else {
        checks_failed++;
        fprintf(stderr, "FAIL: %s\n", label);
    }
}

int main(int argc, char *argv[]) {
    const char *socket_path = argc > 1 ? argv[1] : "/var/run/tric/server.sock";

    TricConnection connection = create_connection(socket_path);
    check("create_connection", check_connection(&connection));
    if (!check_connection(&connection)) {
        fprintf(stderr, "cannot connect to %s\n", socket_path);
        return 1;
    }

    write_value(&connection, (const uint8_t *)"test:1", 6, (const uint8_t *)"hello", 5);
    TricValue value = read_value(&connection, (const uint8_t *)"test:1", 6);
    check("read_value returns data", value.data != NULL);
    check("read_value correct length", value.length == 5);
    check("read_value correct content", value.data && memcmp(value.data, "hello", 5) == 0);
    delete_value_result(&value);

    write_value(&connection, (const uint8_t *)"test:1", 6, (const uint8_t *)"updated", 7);
    value = read_value(&connection, (const uint8_t *)"test:1", 6);
    check("write_value overwrites", value.data && value.length == 7 && memcmp(value.data, "updated", 7) == 0);
    delete_value_result(&value);

    delete_value(&connection, (const uint8_t *)"test:1", 6);
    value = read_value(&connection, (const uint8_t *)"test:1", 6);
    check("delete_value removes key", value.data == NULL);
    delete_value_result(&value);

    write_value(&connection, (const uint8_t *)"test:cas", 8, (const uint8_t *)"original", 8);
    int matched = delete_value_if_match(&connection, (const uint8_t *)"test:cas", 8, (const uint8_t *)"wrong", 5);
    check("delete_value_if_match mismatch returns 0", matched == 0);
    value = read_value(&connection, (const uint8_t *)"test:cas", 8);
    check("delete_value_if_match mismatch keeps value", value.data != NULL);
    delete_value_result(&value);

    matched = delete_value_if_match(&connection, (const uint8_t *)"test:cas", 8, (const uint8_t *)"original", 8);
    check("delete_value_if_match match returns 1", matched == 1);
    value = read_value(&connection, (const uint8_t *)"test:cas", 8);
    check("delete_value_if_match match deletes", value.data == NULL);
    delete_value_result(&value);

    write_value(&connection, (const uint8_t *)"test:ttl", 8, (const uint8_t *)"ephemeral", 9);
    int ttl_result = write_ttl(&connection, (const uint8_t *)"test:ttl", 8, 60000);
    check("write_ttl succeeds", ttl_result == 0);
    value = read_value(&connection, (const uint8_t *)"test:ttl", 8);
    check("write_ttl key still readable", value.data != NULL);
    delete_value_result(&value);
    delete_value(&connection, (const uint8_t *)"test:ttl", 8);

    write_value(&connection, (const uint8_t *)"scan:a", 6, (const uint8_t *)"1", 1);
    write_value(&connection, (const uint8_t *)"scan:b", 6, (const uint8_t *)"2", 1);
    write_value(&connection, (const uint8_t *)"scan:c", 6, (const uint8_t *)"3", 1);
    TricScanResult scan = find_by_prefix(&connection, (const uint8_t *)"scan:", 5);
    check("find_by_prefix returns results", scan.count >= 3);
    delete_scan_result(&scan);
    delete_value(&connection, (const uint8_t *)"scan:a", 6);
    delete_value(&connection, (const uint8_t *)"scan:b", 6);
    delete_value(&connection, (const uint8_t *)"scan:c", 6);

    delete_connection(&connection);
    check("delete_connection", !check_connection(&connection));

    printf("%d passed, %d failed\n", checks_passed, checks_failed);
    return checks_failed > 0 ? 1 : 0;
}
