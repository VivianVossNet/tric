// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: tric-server entry point — creates Core, registers modules, starts supervision.

use std::sync::Arc;

use tric::core::create_core;
use tric::core::data_bus::{create_tric_bus, DataBus};
use tric::modules::cli::{create_cli, CliConfig};
use tric::modules::metrics::create_metrics;
use tric::modules::server::{create_server, ServerConfig};

fn main() {
    let data_bus: Arc<dyn DataBus> = Arc::new(create_tric_bus());
    let metrics = Arc::new(create_metrics());
    let mut core = create_core(data_bus);

    let metrics_for_server = Arc::clone(&metrics);
    core.register_module(move || {
        Box::new(create_server(
            ServerConfig {
                local_path: "/var/run/tric/server.sock".to_string(),
                udp_bind: "0.0.0.0:7483".to_string(),
                max_sessions: 10000,
            },
            Arc::clone(&metrics_for_server),
        ))
    });

    let metrics_for_cli = Arc::clone(&metrics);
    core.register_module(move || {
        Box::new(create_cli(
            CliConfig {
                admin_path: "/var/run/tric/admin.sock".to_string(),
                auth_keys_path: None,
            },
            Arc::clone(&metrics_for_cli),
        ))
    });

    core.run_supervision_loop();
}
