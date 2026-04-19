// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Export — builds .tric (Brotli-compressed tar) or SQL dialect exports from DataBus content.

use std::io::Write;
use std::sync::Arc;

use crate::core::data_bus::DataBus;

pub struct ExportResult {
    pub entries: usize,
    pub bytes_written: usize,
}

pub fn write_tric_archive(
    data_bus: &Arc<dyn DataBus>,
    path: &str,
    debug_uncompressed: bool,
) -> Result<ExportResult, String> {
    let file =
        std::fs::File::create(path).map_err(|error| format!("cannot create {path}: {error}"))?;

    let mut result = ExportResult {
        entries: 0,
        bytes_written: 0,
    };

    if debug_uncompressed {
        let mut archive = tar::Builder::new(file);
        write_tar_contents(&mut archive, data_bus, &mut result)?;
        archive
            .finish()
            .map_err(|error| format!("tar finish failed: {error}"))?;
    } else {
        let encoder = brotli::CompressorWriter::new(file, 4096, 6, 22);
        let mut archive = tar::Builder::new(encoder);
        write_tar_contents(&mut archive, data_bus, &mut result)?;
        archive
            .finish()
            .map_err(|error| format!("tar finish failed: {error}"))?;
    }

    Ok(result)
}

fn write_tar_contents<W: Write>(
    archive: &mut tar::Builder<W>,
    data_bus: &Arc<dyn DataBus>,
    result: &mut ExportResult,
) -> Result<(), String> {
    write_tar_entry(archive, "_meta/version", b"tric+1")?;
    result.entries += 1;

    let all_entries = data_bus.find_by_prefix(b"");

    for (key, value) in &all_entries {
        let key_str = String::from_utf8_lossy(key);

        if let Some(table_name) = key_str.strip_prefix("_schema:") {
            let tar_path = format!("_schema/{table_name}");
            write_tar_entry(archive, &tar_path, value)?;
        } else if let Some(rel_name) = key_str.strip_prefix("_rel:") {
            let tar_path = format!("_rel/{rel_name}");
            write_tar_entry(archive, &tar_path, b"")?;
        } else {
            let tar_path = key_str.replace(':', "/");
            write_tar_entry(archive, &tar_path, value)?;

            if let Some(ttl_remaining) = data_bus.read_ttl_remaining(key) {
                let ttl_ms = ttl_remaining.as_millis().to_string();
                let ttl_path = format!("_ttl/{}", key_str.replace(':', "/"));
                write_tar_entry(archive, &ttl_path, ttl_ms.as_bytes())?;
            }
        }

        result.entries += 1;
        result.bytes_written += key.len() + value.len();
    }

    Ok(())
}

fn write_tar_entry<W: Write>(
    archive: &mut tar::Builder<W>,
    path: &str,
    data: &[u8],
) -> Result<(), String> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    archive
        .append_data(&mut header, path, data)
        .map_err(|error| format!("tar append {path} failed: {error}"))
}

pub fn write_sql_file(
    data_bus: &Arc<dyn DataBus>,
    path: &str,
    dialect: &str,
) -> Result<ExportResult, String> {
    let mut file =
        std::fs::File::create(path).map_err(|error| format!("cannot create {path}: {error}"))?;

    let mut result = ExportResult {
        entries: 0,
        bytes_written: 0,
    };

    let schemas = data_bus.find_by_prefix(b"_schema:");
    for (schema_key, schema_value) in &schemas {
        let table_name = String::from_utf8_lossy(
            schema_key
                .strip_prefix(b"_schema:" as &[u8])
                .unwrap_or(schema_key),
        );
        let schema_str = String::from_utf8_lossy(schema_value);

        let create_statement = build_create_table(&table_name, &schema_str, dialect);
        writeln!(file, "{create_statement}").map_err(|error| format!("write failed: {error}"))?;

        let prefix = format!("{table_name}:");
        let rows = data_bus.find_by_prefix(prefix.as_bytes());
        let columns = parse_schema_columns(&schema_str);

        for (key, value) in &rows {
            let key_str = String::from_utf8_lossy(key);
            let pk_value = &key_str[table_name.len() + 1..];
            let value_str = String::from_utf8_lossy(value);
            let value_parts: Vec<&str> = value_str.split('\n').collect();

            let mut all_values = Vec::new();
            let mut value_index = 0;
            for column in &columns {
                if column.is_pk {
                    all_values.push(format_sql_value(pk_value, &column.data_type, dialect));
                } else if value_index < value_parts.len() {
                    all_values.push(format_sql_value(
                        value_parts[value_index],
                        &column.data_type,
                        dialect,
                    ));
                    value_index += 1;
                }
            }

            let insert = format!(
                "INSERT INTO {} VALUES ({});\n",
                quote_table_name(&table_name, dialect),
                all_values.join(", ")
            );
            write!(file, "{insert}").map_err(|error| format!("write failed: {error}"))?;
            result.entries += 1;
        }
        result.bytes_written += schema_value.len();
    }

    Ok(result)
}

struct SchemaColumn {
    data_type: String,
    is_pk: bool,
}

fn parse_schema_columns(schema: &str) -> Vec<SchemaColumn> {
    schema
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split(':').collect();
            SchemaColumn {
                data_type: parts.get(1).unwrap_or(&"TEXT").to_string(),
                is_pk: parts.contains(&"pk"),
            }
        })
        .collect()
}

fn build_create_table(table_name: &str, schema: &str, dialect: &str) -> String {
    let mut columns = Vec::new();
    let mut pk_columns = Vec::new();

    for line in schema.lines().filter(|line| !line.is_empty()) {
        let parts: Vec<&str> = line.split(':').collect();
        let col_name = parts.first().unwrap_or(&"unknown");
        let col_type = parts.get(1).unwrap_or(&"TEXT");
        let is_pk = parts.contains(&"pk");

        let mapped_type = map_type_to_dialect(col_type, dialect);
        columns.push(format!(
            "    {} {}",
            quote_column_name(col_name, dialect),
            mapped_type
        ));
        if is_pk {
            pk_columns.push(col_name.to_string());
        }
    }

    if !pk_columns.is_empty() {
        let pk_list = pk_columns
            .iter()
            .map(|column| quote_column_name(column, dialect))
            .collect::<Vec<_>>()
            .join(", ");
        columns.push(format!("    PRIMARY KEY ({pk_list})"));
    }

    format!(
        "CREATE TABLE {} (\n{}\n);",
        quote_table_name(table_name, dialect),
        columns.join(",\n")
    )
}

fn map_type_to_dialect(tric_type: &str, dialect: &str) -> String {
    match (tric_type.to_uppercase().as_str(), dialect) {
        ("INT" | "INTEGER", "sqlite") => "INTEGER".to_string(),
        ("BOOLEAN", "mysql") => "TINYINT(1)".to_string(),
        ("BOOLEAN", "sqlite") => "INTEGER".to_string(),
        ("TIMESTAMP" | "DATETIME", "sqlite") => "TEXT".to_string(),
        _ => tric_type.to_string(),
    }
}

fn quote_table_name(name: &str, dialect: &str) -> String {
    match dialect {
        "mysql" => format!("`{name}`"),
        _ => name.to_string(),
    }
}

fn quote_column_name(name: &str, dialect: &str) -> String {
    match dialect {
        "mysql" => format!("`{name}`"),
        "postgres" => format!("\"{name}\""),
        _ => name.to_string(),
    }
}

fn format_sql_value(value: &str, data_type: &str, _dialect: &str) -> String {
    let upper_type = data_type.to_uppercase();
    if upper_type.starts_with("INT")
        || upper_type.starts_with("DECIMAL")
        || upper_type.starts_with("NUMERIC")
        || upper_type.starts_with("REAL")
        || upper_type.starts_with("FLOAT")
        || upper_type.starts_with("DOUBLE")
    {
        if value.is_empty() {
            "NULL".to_string()
        } else {
            value.to_string()
        }
    } else if value.is_empty() {
        "NULL".to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}
