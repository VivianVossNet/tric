# SPDX-License-Identifier: BSD-3-Clause
# Copyright (c) 2025-2026 Vivian Voss
# Scope: Integration test for the TRIC+ Tcl bridge — verifies all six primitives against a running server.

package require tcltest
namespace import ::tcltest::*

lappend auto_path [file dirname [file dirname [file normalize [info script]]]]
package require tric

set socketPath [expr {[info exists ::env(TRIC_SOCKET)] ? $::env(TRIC_SOCKET) : "/tmp/tric-tcl-test/server.sock"}]
set h [tric::connect $socketPath]

test connection-1.1 "connection is valid" -body {
    tric::valid $h
} -result 1

test read-1.1 "read returns written value" -body {
    tric::write $h "test:1" "hello"
    set v [tric::read $h "test:1"]
    expr {$v ne ""}
} -result 1

test read-1.2 "read returns correct length" -body {
    tric::write $h "test:len" "hello"
    set v [tric::read $h "test:len"]
    string length $v
} -result 5

test read-1.3 "read returns correct content" -body {
    tric::write $h "test:content" "hello"
    tric::read $h "test:content"
} -result "hello"

test write-1.1 "write overwrites" -body {
    tric::write $h "test:over" "original"
    tric::write $h "test:over" "updated"
    tric::read $h "test:over"
} -result "updated"

test del-1.1 "del removes key" -body {
    tric::write $h "test:del" "payload"
    tric::del $h "test:del"
    tric::read $h "test:del"
} -result ""

test cad-1.1 "cad mismatch returns false" -body {
    tric::write $h "test:cas-miss" "original"
    tric::cad $h "test:cas-miss" "wrong"
} -result 0

test cad-1.2 "cad mismatch keeps value" -body {
    tric::write $h "test:cas-keep" "original"
    tric::cad $h "test:cas-keep" "wrong"
    set v [tric::read $h "test:cas-keep"]
    expr {$v ne ""}
} -result 1

test cad-1.3 "cad match returns true" -body {
    tric::write $h "test:cas-match" "original"
    tric::cad $h "test:cas-match" "original"
} -result 1

test cad-1.4 "cad match deletes" -body {
    tric::write $h "test:cas-del" "original"
    tric::cad $h "test:cas-del" "original"
    tric::read $h "test:cas-del"
} -result ""

test ttl-1.1 "ttl succeeds" -body {
    tric::write $h "test:ttl" "ephemeral"
    tric::ttl $h "test:ttl" 60000
} -result ""

test ttl-1.2 "ttl key still readable" -body {
    tric::write $h "test:ttl-read" "ephemeral"
    tric::ttl $h "test:ttl-read" 60000
    set v [tric::read $h "test:ttl-read"]
    expr {$v ne ""}
} -result 1

test scan-1.1 "scan returns results" -body {
    tric::write $h "scan:a" "1"
    tric::write $h "scan:b" "2"
    tric::write $h "scan:c" "3"
    set pairs [tric::scan $h "scan:"]
    set count [expr {[llength $pairs] / 2}]
    tric::del $h "scan:a"
    tric::del $h "scan:b"
    tric::del $h "scan:c"
    expr {$count >= 3}
} -result 1

test round-trip-1.1 "round-trip varied bytes" -body {
    tric::write $h "test:slice" "value with spaces and more bytes"
    tric::read $h "test:slice"
} -result "value with spaces and more bytes"

tric::disconnect $h
cleanupTests
