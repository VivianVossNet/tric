// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: Integration test for the TRIC+ Swift bridge — verifies all six primitives against a running server.

import Foundation
import XCTest

@testable import Tric

final class BridgeTest: XCTestCase {
  private static var connection: Connection!
  private static var serverProcess: Process?
  private static var socketPath: String = ""

  override class func setUp() {
    super.setUp()
    let tempDir = "/tmp/tric-swift-test-\(ProcessInfo.processInfo.processIdentifier)"
    try? FileManager.default.removeItem(atPath: tempDir)
    try? FileManager.default.createDirectory(atPath: tempDir, withIntermediateDirectories: true)
    socketPath = "\(tempDir)/server.sock"

    let process = Process()
    process.executableURL = URL(fileURLWithPath: findTricBinary())
    process.arguments = ["server"]
    process.environment = [
      "TRIC_SOCKET_DIR": tempDir,
      "TRIC_BASE_DIR": "\(tempDir)/data",
      "TRIC_INSTANCE": "swifttest",
      "TRIC_SLOT": "0",
    ]
    process.standardOutput = FileHandle(forWritingAtPath: "/dev/null")
    process.standardError = FileHandle(forWritingAtPath: "/dev/null")

    do {
      try process.run()
      serverProcess = process
    } catch {
      XCTFail("failed to start tric server: \(error)")
      return
    }

    Thread.sleep(forTimeInterval: 2.0)
    connection = Connection(socketPath: socketPath)
  }

  override class func tearDown() {
    connection = nil
    serverProcess?.terminate()
    serverProcess?.waitUntilExit()
    let tempDir = (socketPath as NSString).deletingLastPathComponent
    try? FileManager.default.removeItem(atPath: tempDir)
    super.tearDown()
  }

  private static func findTricBinary() -> String {
    let candidates = [
      ProcessInfo.processInfo.environment["TRIC_BINARY"],
      "./target/release/tric",
      "../../target/release/tric",
      "../../../target/release/tric",
    ].compactMap { $0 }
    for candidate in candidates {
      if FileManager.default.isExecutableFile(atPath: candidate) {
        return candidate
      }
    }
    XCTFail("tric server binary not found; set TRIC_BINARY or build with `cargo build --release`")
    return "/dev/null"
  }

  func testConnectionIsValid() {
    XCTAssertTrue(Self.connection.isValid, "connection should be valid")
  }

  func testReadReturnsWrittenValue() throws {
    try Self.connection.write("test:1", "hello")
    let value = Self.connection.read("test:1")
    XCTAssertNotNil(value, "read should return data")
  }

  func testReadReturnsCorrectLength() throws {
    try Self.connection.write("test:len", "hello")
    let value = Self.connection.read("test:len")
    XCTAssertEqual(value?.count, 5, "read should return 5 bytes")
  }

  func testReadReturnsCorrectContent() throws {
    try Self.connection.write("test:content", "hello")
    let value = Self.connection.read("test:content")
    XCTAssertEqual(value, Data("hello".utf8), "read should return exact content")
  }

  func testWriteOverwrites() throws {
    try Self.connection.write("test:over", "original")
    try Self.connection.write("test:over", "updated")
    let value = Self.connection.read("test:over")
    XCTAssertEqual(value, Data("updated".utf8), "write should overwrite")
  }

  func testDelRemovesKey() throws {
    try Self.connection.write("test:del", "payload")
    try Self.connection.del("test:del")
    let value = Self.connection.read("test:del")
    XCTAssertNil(value, "del should remove the key")
  }

  func testCadMismatchReturnsFalse() throws {
    try Self.connection.write("test:cas-miss", "original")
    let matched = try Self.connection.cad("test:cas-miss", expected: "wrong")
    XCTAssertFalse(matched, "cad with wrong expected should return false")
  }

  func testCadMismatchKeepsValue() throws {
    try Self.connection.write("test:cas-keep", "original")
    _ = try Self.connection.cad("test:cas-keep", expected: "wrong")
    let value = Self.connection.read("test:cas-keep")
    XCTAssertNotNil(value, "cad mismatch should leave the value intact")
  }

  func testCadMatchReturnsTrue() throws {
    try Self.connection.write("test:cas-match", "original")
    let matched = try Self.connection.cad("test:cas-match", expected: "original")
    XCTAssertTrue(matched, "cad with correct expected should return true")
  }

  func testCadMatchDeletes() throws {
    try Self.connection.write("test:cas-del", "original")
    _ = try Self.connection.cad("test:cas-del", expected: "original")
    let value = Self.connection.read("test:cas-del")
    XCTAssertNil(value, "cad match should delete the key")
  }

  func testTtlSucceeds() throws {
    try Self.connection.write("test:ttl", "ephemeral")
    XCTAssertNoThrow(try Self.connection.ttl("test:ttl", durationMs: 60_000))
  }

  func testTtlKeyStillReadable() throws {
    try Self.connection.write("test:ttl-read", "ephemeral")
    try Self.connection.ttl("test:ttl-read", durationMs: 60_000)
    let value = Self.connection.read("test:ttl-read")
    XCTAssertNotNil(value, "ttl key should still be readable before expiry")
  }

  func testScanReturnsResults() throws {
    try Self.connection.write("scan:a", "1")
    try Self.connection.write("scan:b", "2")
    try Self.connection.write("scan:c", "3")
    let pairs = Self.connection.scan("scan:")
    XCTAssertGreaterThanOrEqual(pairs.count, 3, "scan should return at least 3 pairs")
    try Self.connection.del("scan:a")
    try Self.connection.del("scan:b")
    try Self.connection.del("scan:c")
  }

  func testStringConvenienceOverloads() throws {
    try Self.connection.write("test:str", "value")
    let value = Self.connection.read("test:str")
    XCTAssertEqual(value, Data("value".utf8), "string overload should round-trip")
  }
}
