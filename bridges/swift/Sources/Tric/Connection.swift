// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: TRIC+ Swift client — RAII Connection class wrapping the C bridge via CTric FFI.

import CTric
import Foundation

public final class Connection {
  private var handle: TricConnection
  private var owned: Bool

  public init(socketPath: String) {
    self.handle = socketPath.withCString { cpath in
      create_connection(cpath)
    }
    self.owned = handle.socket_fd >= 0
  }

  deinit {
    if owned {
      delete_connection(&handle)
    }
  }

  public var isValid: Bool {
    owned && check_connection(&handle) != 0
  }

  public func read(_ key: Data) -> Data? {
    let value: TricValue = key.withUnsafeBytes { keyBytes in
      read_value(&handle, keyBytes.bindMemory(to: UInt8.self).baseAddress, key.count)
    }
    guard let dataPtr = value.data else {
      return nil
    }
    let out = Data(bytes: dataPtr, count: value.length)
    var v = value
    delete_value_result(&v)
    return out
  }

  public func read(_ key: String) -> Data? {
    read(Data(key.utf8))
  }

  public func write(_ key: Data, _ value: Data) throws {
    let result = key.withUnsafeBytes { keyBytes in
      value.withUnsafeBytes { valueBytes in
        write_value(
          &handle,
          keyBytes.bindMemory(to: UInt8.self).baseAddress, key.count,
          valueBytes.bindMemory(to: UInt8.self).baseAddress, value.count
        )
      }
    }
    if result != 0 {
      throw TricError.communication
    }
  }

  public func write(_ key: String, _ value: String) throws {
    try write(Data(key.utf8), Data(value.utf8))
  }

  public func del(_ key: Data) throws {
    let result = key.withUnsafeBytes { keyBytes in
      delete_value(&handle, keyBytes.bindMemory(to: UInt8.self).baseAddress, key.count)
    }
    if result != 0 {
      throw TricError.communication
    }
  }

  public func del(_ key: String) throws {
    try del(Data(key.utf8))
  }

  public func cad(_ key: Data, expected: Data) throws -> Bool {
    let result = key.withUnsafeBytes { keyBytes in
      expected.withUnsafeBytes { expectedBytes in
        delete_value_if_match(
          &handle,
          keyBytes.bindMemory(to: UInt8.self).baseAddress, key.count,
          expectedBytes.bindMemory(to: UInt8.self).baseAddress, expected.count
        )
      }
    }
    if result < 0 {
      throw TricError.communication
    }
    return result == 1
  }

  public func cad(_ key: String, expected: String) throws -> Bool {
    try cad(Data(key.utf8), expected: Data(expected.utf8))
  }

  public func ttl(_ key: Data, durationMs: UInt64) throws {
    let result = key.withUnsafeBytes { keyBytes in
      write_ttl(&handle, keyBytes.bindMemory(to: UInt8.self).baseAddress, key.count, durationMs)
    }
    if result != 0 {
      throw TricError.communication
    }
  }

  public func ttl(_ key: String, durationMs: UInt64) throws {
    try ttl(Data(key.utf8), durationMs: durationMs)
  }

  public func scan(_ prefix: Data) -> [(Data, Data)] {
    let result: TricScanResult = prefix.withUnsafeBytes { prefixBytes in
      find_by_prefix(&handle, prefixBytes.bindMemory(to: UInt8.self).baseAddress, prefix.count)
    }
    var pairs: [(Data, Data)] = []
    pairs.reserveCapacity(result.count)
    for i in 0..<result.count {
      let pair = result.pairs[i]
      let key = Data(bytes: pair.key, count: pair.key_length)
      let value = Data(bytes: pair.value, count: pair.value_length)
      pairs.append((key, value))
    }
    var r = result
    delete_scan_result(&r)
    return pairs
  }

  public func scan(_ prefix: String) -> [(Data, Data)] {
    scan(Data(prefix.utf8))
  }
}
