// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: tric-server entry point — creates Core, registers modules, starts supervision.

use std::sync::Arc;

use tric::core::create_core;
use tric::core::data_bus::{create_tric_bus, DataBus};
use tric::modules::server::{create_server, ServerConfig};

fn main() {
    let data_bus: Arc<dyn DataBus> = Arc::new(create_tric_bus());
    let mut core = create_core(data_bus);

    core.register_module(|| {
        Box::new(create_server(ServerConfig {
            local_path: "/var/run/tric/server.sock".to_string(),
        }))
    });

    core.run_supervision_loop();
}
