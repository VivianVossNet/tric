// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Logger — BSD syslog (RFC 3164) writer via local syslog socket, structured key-value fields.

use std::io::Write;
use std::os::unix::net::UnixDatagram;
use std::sync::OnceLock;
use std::time::SystemTime;

static SYSLOG_SOCKET: OnceLock<Option<UnixDatagram>> = OnceLock::new();

const FACILITY_DAEMON: u8 = 3;
const SEVERITY_INFO: u8 = 6;
const SEVERITY_WARNING: u8 = 4;
const SEVERITY_ERROR: u8 = 3;

fn create_syslog_connection() -> Option<UnixDatagram> {
    let socket = UnixDatagram::unbound().ok()?;
    let paths = ["/dev/log", "/var/run/log", "/var/run/syslog"];
    for path in &paths {
        if socket.connect(path).is_ok() {
            return Some(socket);
        }
    }
    None
}

fn write_syslog(severity: u8, message: &str) {
    let socket = SYSLOG_SOCKET.get_or_init(create_syslog_connection);
    let Some(socket) = socket.as_ref() else {
        return;
    };

    let priority = (FACILITY_DAEMON as u16) * 8 + severity as u16;
    let timestamp = format_timestamp();
    let mut buffer = Vec::with_capacity(256);
    let _ = write!(buffer, "<{priority}>{timestamp} tric-server: {message}");

    let _ = socket.send(&buffer);
}

fn format_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let (year, month, day) = days_to_date(days);
    let month_name = match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    };
    let _ = year;
    format!("{month_name} {day:>2} {hours:02}:{minutes:02}:{seconds:02}")
}

fn days_to_date(days_since_epoch: u64) -> (u64, u8, u8) {
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let day_of_era = z - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month + 2) / 5 + 1;
    let month = if month < 10 { month + 3 } else { month - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u8, day as u8)
}

pub fn log_info(message: &str) {
    write_syslog(SEVERITY_INFO, message);
}

pub fn log_warning(message: &str) {
    write_syslog(SEVERITY_WARNING, message);
}

pub fn log_error(message: &str) {
    write_syslog(SEVERITY_ERROR, message);
}
