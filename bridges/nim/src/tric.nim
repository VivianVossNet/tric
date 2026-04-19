# SPDX-License-Identifier: BSD-3-Clause
# Copyright (c) 2025-2026 Vivian Voss
# Scope: TRIC+ Nim client — Connection object wrapping the C bridge via importc + compile pragmas.

import std/[options, os]

const cDir = currentSourcePath().parentDir().parentDir() / ".." / "c"

{.passc: "-I" & cDir.}
{.compile: cDir / "tric.c".}

type
  CTricConnection {.importc: "TricConnection", header: "tric.h",
      bycopy.} = object
    socketFd {.importc: "socket_fd".}: cint
    requestCounter {.importc: "request_counter".}: uint32

  CTricValue {.importc: "TricValue", header: "tric.h", bycopy.} = object
    data: ptr uint8
    length: csize_t

  CTricPair {.importc: "TricPair", header: "tric.h", bycopy.} = object
    key: ptr uint8
    keyLength {.importc: "key_length".}: csize_t
    value: ptr uint8
    valueLength {.importc: "value_length".}: csize_t

  CTricScanResult {.importc: "TricScanResult", header: "tric.h",
      bycopy.} = object
    pairs: ptr CTricPair
    count: csize_t

proc createConnection(socketPath: cstring): CTricConnection {.importc: "create_connection",
    header: "tric.h".}
proc deleteConnection(conn: ptr CTricConnection) {.importc: "delete_connection",
    header: "tric.h".}
proc checkConnection(conn: ptr CTricConnection): cint {.importc: "check_connection",
    header: "tric.h".}

proc cReadValue(conn: ptr CTricConnection, key: ptr uint8,
    keyLen: csize_t): CTricValue {.importc: "read_value", header: "tric.h".}
proc cWriteValue(conn: ptr CTricConnection, key: ptr uint8, keyLen: csize_t,
    value: ptr uint8, valueLen: csize_t): cint {.importc: "write_value",
    header: "tric.h".}
proc cDeleteValue(conn: ptr CTricConnection, key: ptr uint8,
    keyLen: csize_t): cint {.importc: "delete_value", header: "tric.h".}
proc cDeleteValueIfMatch(conn: ptr CTricConnection, key: ptr uint8,
    keyLen: csize_t, expected: ptr uint8,
    expectedLen: csize_t): cint {.importc: "delete_value_if_match",
    header: "tric.h".}
proc cWriteTtl(conn: ptr CTricConnection, key: ptr uint8, keyLen: csize_t,
    durationMs: uint64): cint {.importc: "write_ttl", header: "tric.h".}
proc cFindByPrefix(conn: ptr CTricConnection, prefix: ptr uint8,
    prefixLen: csize_t): CTricScanResult {.importc: "find_by_prefix",
    header: "tric.h".}

proc deleteValueResult(v: ptr CTricValue) {.importc: "delete_value_result",
    header: "tric.h".}
proc deleteScanResult(r: ptr CTricScanResult) {.importc: "delete_scan_result",
    header: "tric.h".}

type
  TricError* = object of CatchableError

  TricConnection* = object
    handle: CTricConnection
    owned: bool

  TricPair* = tuple[key, value: string]

proc `=destroy`(conn: var TricConnection) =
  if conn.owned:
    deleteConnection(addr conn.handle)
    conn.owned = false

proc `=copy`(dest: var TricConnection, source: TricConnection) {.error.}

proc initConnection*(socketPath: string): TricConnection =
  result.handle = createConnection(socketPath.cstring)
  result.owned = result.handle.socketFd >= 0

proc isValid*(conn: var TricConnection): bool =
  conn.owned and checkConnection(addr conn.handle) != 0

proc bytePtr(s: string): ptr uint8 =
  if s.len == 0:
    return nil
  return cast[ptr uint8](unsafeAddr s[0])

proc read*(conn: var TricConnection, key: string): Option[string] =
  var value = cReadValue(addr conn.handle, bytePtr(key), csize_t(key.len))
  if value.data == nil:
    return none(string)
  var outStr = newString(value.length)
  if value.length > 0:
    copyMem(addr outStr[0], value.data, value.length)
  deleteValueResult(addr value)
  some(outStr)

proc write*(conn: var TricConnection, key, value: string) =
  if cWriteValue(addr conn.handle, bytePtr(key), csize_t(key.len), bytePtr(
      value), csize_t(value.len)) != 0:
    raise newException(TricError, "write failed")

proc del*(conn: var TricConnection, key: string) =
  if cDeleteValue(addr conn.handle, bytePtr(key), csize_t(key.len)) != 0:
    raise newException(TricError, "del failed")

proc cad*(conn: var TricConnection, key, expected: string): bool =
  let resultCode = cDeleteValueIfMatch(addr conn.handle, bytePtr(key), csize_t(
      key.len), bytePtr(expected), csize_t(expected.len))
  if resultCode < 0:
    raise newException(TricError, "cad failed")
  resultCode == 1

proc ttl*(conn: var TricConnection, key: string, durationMs: uint64) =
  if cWriteTtl(addr conn.handle, bytePtr(key), csize_t(key.len), durationMs) != 0:
    raise newException(TricError, "ttl failed")

proc scan*(conn: var TricConnection, prefix: string): seq[TricPair] =
  var scanResult = cFindByPrefix(addr conn.handle, bytePtr(prefix), csize_t(prefix.len))
  result = newSeqOfCap[TricPair](scanResult.count)
  let pairsPtr = scanResult.pairs
  for i in 0 ..< scanResult.count:
    let src = cast[ptr CTricPair](cast[uint](pairsPtr) + uint(i) * uint(sizeof(
        CTricPair)))[]
    var keyStr = newString(src.keyLength)
    var valueStr = newString(src.valueLength)
    if src.keyLength > 0:
      copyMem(addr keyStr[0], src.key, src.keyLength)
    if src.valueLength > 0:
      copyMem(addr valueStr[0], src.value, src.valueLength)
    result.add((key: keyStr, value: valueStr))
  deleteScanResult(addr scanResult)
