# SPDX-License-Identifier: BSD-3-Clause
# Copyright (c) 2025-2026 Vivian Voss
# Scope: TRIC+ Nim client — nimble package manifest.

version       = "0.0.0"
author        = "Vivian Voss"
description   = "TRIC+ Permutive Database Engine — Nim client bridge"
license       = "BSD-3-Clause"
srcDir        = "src"

requires "nim >= 2.0.0"

task test, "Run integration tests against a running TRIC+ server":
  exec "nim c --hints:off --warnings:off -r tests/bridge_test.nim"
