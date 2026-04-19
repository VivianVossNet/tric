// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: TRIC+ Swift client — error types thrown by Connection methods.

import Foundation

public enum TricError: Error, Equatable {
  case connectionInvalid
  case communication
}
