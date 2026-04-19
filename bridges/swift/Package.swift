// swift-tools-version:5.9
// Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause
// Scope: TRIC+ Swift client — SPM manifest wiring CTric (C bridge) + Tric (Swift wrapper) + TricTests.

import PackageDescription

let package = Package(
    name: "Tric",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .library(name: "Tric", targets: ["Tric"])
    ],
    targets: [
        .target(
            name: "CTric",
            path: "Sources/CTric",
            publicHeadersPath: "include"
        ),
        .target(
            name: "Tric",
            dependencies: ["CTric"],
            path: "Sources/Tric"
        ),
        .testTarget(
            name: "TricTests",
            dependencies: ["Tric"],
            path: "Tests/TricTests"
        )
    ],
    swiftLanguageVersions: [.v5]
)
