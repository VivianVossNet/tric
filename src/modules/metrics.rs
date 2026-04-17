// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Metrics — lock-free atomic counters for operations, errors, sessions, and latency tracking.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct Metrics {
    requests_total: AtomicU64,
    requests_local: AtomicU64,
    requests_network: AtomicU64,
    errors_total: AtomicU64,
    active_sessions: AtomicU64,
    latency_sum_microseconds: AtomicU64,
    latency_max_microseconds: AtomicU64,
}

pub fn create_metrics() -> Metrics {
    Metrics {
        requests_total: AtomicU64::new(0),
        requests_local: AtomicU64::new(0),
        requests_network: AtomicU64::new(0),
        errors_total: AtomicU64::new(0),
        active_sessions: AtomicU64::new(0),
        latency_sum_microseconds: AtomicU64::new(0),
        latency_max_microseconds: AtomicU64::new(0),
    }
}

impl Metrics {
    pub fn record_local_request(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_local.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_network_request(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        self.requests_network.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_latency(&self, start: Instant) {
        let microseconds = start.elapsed().as_micros() as u64;
        self.latency_sum_microseconds
            .fetch_add(microseconds, Ordering::Relaxed);
        self.latency_max_microseconds
            .fetch_max(microseconds, Ordering::Relaxed);
    }

    pub fn read_requests_total(&self) -> u64 {
        self.requests_total.load(Ordering::Relaxed)
    }

    pub fn read_requests_local(&self) -> u64 {
        self.requests_local.load(Ordering::Relaxed)
    }

    pub fn read_requests_network(&self) -> u64 {
        self.requests_network.load(Ordering::Relaxed)
    }

    pub fn read_errors_total(&self) -> u64 {
        self.errors_total.load(Ordering::Relaxed)
    }

    pub fn read_active_sessions(&self) -> u64 {
        self.active_sessions.load(Ordering::Relaxed)
    }

    pub fn read_latency_average_microseconds(&self) -> u64 {
        let total = self.requests_total.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }
        self.latency_sum_microseconds.load(Ordering::Relaxed) / total
    }

    pub fn read_latency_max_microseconds(&self) -> u64 {
        self.latency_max_microseconds.load(Ordering::Relaxed)
    }

    pub fn increment_sessions(&self) {
        self.active_sessions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_sessions(&self) {
        self.active_sessions.fetch_sub(1, Ordering::Relaxed);
    }
}
