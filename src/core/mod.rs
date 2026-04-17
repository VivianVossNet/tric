// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Core — module registry, thread spawning, supervision loop with heartbeat detection and respawn.

pub mod data_bus;
pub mod module;
pub mod permutive_bus;
pub mod sqlite_bus;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::{create_tric, Tric};

use data_bus::DataBus;
use module::{Module, ModuleContext};

type ModuleFactory = Box<dyn Fn() -> Box<dyn Module> + Send + Sync>;

struct RegisteredModule {
    name: &'static str,
    factory: Arc<ModuleFactory>,
    handle: Option<thread::JoinHandle<()>>,
}

pub struct Core {
    core_bus: Tric,
    data_bus: Arc<dyn DataBus>,
    modules: Vec<RegisteredModule>,
    supervision_interval: Duration,
    heartbeat_ttl: Duration,
}

pub fn create_core(data_bus: Arc<dyn DataBus>) -> Core {
    Core {
        core_bus: create_tric(),
        data_bus,
        modules: Vec::new(),
        supervision_interval: Duration::from_secs(5),
        heartbeat_ttl: Duration::from_secs(15),
    }
}

fn create_module_thread(
    name: &'static str,
    factory: &Arc<ModuleFactory>,
    core_bus: Tric,
    data_bus: Arc<dyn DataBus>,
    heartbeat_ttl: Duration,
) -> thread::JoinHandle<()> {
    let factory = Arc::clone(factory);

    thread::spawn(move || {
        let module_key = format!("module:{name}");
        core_bus.write_value(module_key.as_bytes(), b"starting");
        core_bus.write_ttl(module_key.as_bytes(), heartbeat_ttl);

        let context = ModuleContext {
            core_bus: core_bus.clone(),
            data_bus,
        };

        let module = factory();
        core_bus.write_value(module_key.as_bytes(), b"running");
        core_bus.write_ttl(module_key.as_bytes(), heartbeat_ttl);

        module.run(context);
    })
}

impl Core {
    pub fn register_module<F>(&mut self, factory: F)
    where
        F: Fn() -> Box<dyn Module> + Send + Sync + 'static,
    {
        let module = factory();
        let name = module.name();
        let factory = Arc::new(Box::new(factory) as ModuleFactory);
        let handle = create_module_thread(
            name,
            &factory,
            self.core_bus.clone(),
            Arc::clone(&self.data_bus),
            self.heartbeat_ttl,
        );
        self.modules.push(RegisteredModule {
            name,
            factory,
            handle: Some(handle),
        });
    }

    pub fn run_supervision_loop(&mut self) {
        loop {
            thread::sleep(self.supervision_interval);
            self.check_and_respawn_modules();
        }
    }

    fn check_and_respawn_modules(&mut self) {
        let alive = self.core_bus.find_by_prefix(b"module:");
        let alive_names: Vec<&[u8]> = alive.iter().map(|(key, _)| key.as_ref()).collect();

        let core_bus = self.core_bus.clone();
        let data_bus = Arc::clone(&self.data_bus);
        let heartbeat_ttl = self.heartbeat_ttl;

        for registered in &mut self.modules {
            let module_key = format!("module:{}", registered.name);

            if !alive_names.contains(&module_key.as_bytes()) {
                if let Some(handle) = registered.handle.take() {
                    let _ = handle.join();
                }
                registered.handle = Some(create_module_thread(
                    registered.name,
                    &registered.factory,
                    core_bus.clone(),
                    Arc::clone(&data_bus),
                    heartbeat_ttl,
                ));
            }
        }
    }
}
