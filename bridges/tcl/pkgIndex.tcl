# SPDX-License-Identifier: BSD-3-Clause
# Copyright (c) 2025-2026 Vivian Voss
# Scope: Package index declaring the TRIC+ Tcl extension.

package ifneeded tric 0.0.0 [list load [file join $dir libtric_tcl[info sharedlibextension]] Tric]
