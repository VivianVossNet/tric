#!/bin/sh
# Copyright 2025-2026 Vivian Voss. Licensed under the BSD 3-Clause License.
# SPDX-License-Identifier: BSD-3-Clause
# Scope: Build the TRIC+ Tcl loadable extension (libtric_tcl.dylib / .so).

set -eu

TCLTK_PREFIX="${TCLTK_PREFIX:-/opt/homebrew/opt/tcl-tk}"
TCL_INCLUDE="$TCLTK_PREFIX/include/tcl-tk"
TCL_LIB="$TCLTK_PREFIX/lib"
BRIDGES_C="$(cd "$(dirname "$0")/../c" && pwd)"
HERE="$(cd "$(dirname "$0")" && pwd)"

case "$(uname -s)" in
    Darwin) EXT=dylib ;;
    *)      EXT=so ;;
esac

cc -std=c11 -Wall -Wextra -Wpedantic -Werror -O2 -fPIC \
    -DUSE_TCL_STUBS \
    -I"$TCL_INCLUDE" -I"$BRIDGES_C" \
    -shared \
    -o "$HERE/libtric_tcl.$EXT" \
    "$HERE/tric_tcl.c" "$BRIDGES_C/tric.c" \
    -L"$TCL_LIB" -ltclstub

echo "built $HERE/libtric_tcl.$EXT"
