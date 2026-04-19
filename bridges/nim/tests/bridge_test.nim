# SPDX-License-Identifier: BSD-3-Clause
# Copyright (c) 2025-2026 Vivian Voss
# Scope: Integration test for the TRIC+ Nim bridge — verifies all six primitives against a running server.

import std/[options, os, unittest]
import ../src/tric

const defaultSocketPath = "/tmp/tric-nim-test/server.sock"

proc resolveSocketPath(): string =
  let env = getEnv("TRIC_SOCKET")
  if env.len > 0: env else: defaultSocketPath

suite "TRIC+ Nim bridge":
  var connection = initConnection(resolveSocketPath())
  require connection.isValid()

  test "connection is valid":
    check connection.isValid()

  test "read returns written value":
    connection.write("test:1", "hello")
    check connection.read("test:1").isSome()

  test "read returns correct length":
    connection.write("test:len", "hello")
    let value = connection.read("test:len").get()
    check value.len == 5

  test "read returns correct content":
    connection.write("test:content", "hello")
    check connection.read("test:content").get() == "hello"

  test "write overwrites":
    connection.write("test:over", "original")
    connection.write("test:over", "updated")
    check connection.read("test:over").get() == "updated"

  test "del removes key":
    connection.write("test:del", "payload")
    connection.del("test:del")
    check connection.read("test:del").isNone()

  test "cad mismatch returns false":
    connection.write("test:cas-miss", "original")
    check not connection.cad("test:cas-miss", "wrong")

  test "cad mismatch keeps value":
    connection.write("test:cas-keep", "original")
    discard connection.cad("test:cas-keep", "wrong")
    check connection.read("test:cas-keep").isSome()

  test "cad match returns true":
    connection.write("test:cas-match", "original")
    check connection.cad("test:cas-match", "original")

  test "cad match deletes":
    connection.write("test:cas-del", "original")
    discard connection.cad("test:cas-del", "original")
    check connection.read("test:cas-del").isNone()

  test "ttl succeeds":
    connection.write("test:ttl", "ephemeral")
    connection.ttl("test:ttl", 60_000'u64)

  test "ttl key still readable":
    connection.write("test:ttl-read", "ephemeral")
    connection.ttl("test:ttl-read", 60_000'u64)
    check connection.read("test:ttl-read").isSome()

  test "scan returns results":
    connection.write("scan:a", "1")
    connection.write("scan:b", "2")
    connection.write("scan:c", "3")
    let pairs = connection.scan("scan:")
    check pairs.len >= 3
    connection.del("scan:a")
    connection.del("scan:b")
    connection.del("scan:c")

  test "round-trip varied bytes":
    let value = "value with spaces and more bytes"
    connection.write("test:slice", value)
    check connection.read("test:slice").get() == value
