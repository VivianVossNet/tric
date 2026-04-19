// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Import — reads SQL dump, applies Storage Plan, bulk-writes data and schema to DataBus.

use std::sync::Arc;

use sqlparser::ast::{Expr, SetExpr, Statement, Value};
use sqlparser::dialect::{GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

use crate::core::data_bus::DataBus;
use crate::modules::analyser::{render_schema_entry, StoragePlan, TablePlan};

pub fn parse_sql(content: &str, format: &str) -> Vec<Statement> {
    let dialect: Box<dyn sqlparser::dialect::Dialect> = match format {
        "mysql" => Box::new(MySqlDialect {}),
        "postgres" => Box::new(PostgreSqlDialect {}),
        "sqlite" => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    };
    Parser::parse_sql(&*dialect, content).unwrap_or_default()
}

pub fn parse_import(
    statements: &[Statement],
    plan: &StoragePlan,
    data_bus: &Arc<dyn DataBus>,
) -> ImportResult {
    let mut result = ImportResult {
        tables: 0,
        rows: 0,
        relationships: 0,
        errors: 0,
    };

    for table in &plan.tables {
        let schema_key = format!("_schema:{}", table.name);
        let schema_value = render_schema_entry(table);
        data_bus.write_value(schema_key.as_bytes(), schema_value.as_bytes());
        result.tables += 1;
    }

    for statement in statements {
        if let Statement::Insert(insert) = statement {
            let table_name = insert.table_name.to_string().replace(['`', '"'], "");
            let Some(table_plan) = plan.tables.iter().find(|table| table.name == table_name) else {
                continue;
            };

            if let Some(source) = &insert.source {
                if let SetExpr::Values(values) = source.body.as_ref() {
                    for row in &values.rows {
                        match write_row(table_plan, row, data_bus) {
                            Ok(relationship_count) => {
                                result.rows += 1;
                                result.relationships += relationship_count;
                            }
                            Err(_) => result.errors += 1,
                        }
                    }
                }
            }
        }
    }

    result
}

fn write_row(table: &TablePlan, values: &[Expr], data_bus: &Arc<dyn DataBus>) -> Result<usize, ()> {
    if values.len() != table.columns.len() {
        return Err(());
    }

    let key_parts: Vec<String> = table
        .primary_key
        .iter()
        .filter_map(|pk_name| {
            let index = table
                .columns
                .iter()
                .position(|column| column.name == *pk_name)?;
            Some(parse_value(&values[index]))
        })
        .collect();

    if key_parts.is_empty() {
        return Err(());
    }

    let key = format!("{}:{}", table.name, key_parts.join(":"));

    let mut value_parts = Vec::new();
    for (index, column) in table.columns.iter().enumerate() {
        if column.is_primary_key {
            continue;
        }
        value_parts.push(parse_value(&values[index]));
    }
    let value = value_parts.join("\n");

    data_bus.write_value(key.as_bytes(), value.as_bytes());

    let mut relationship_count = 0;
    for fk in &table.foreign_keys {
        if let Some(fk_index) = table
            .columns
            .iter()
            .position(|column| column.name == fk.column)
        {
            let fk_value = parse_value(&values[fk_index]);
            let rel_key = format!(
                "_rel:{}:{}:{}:{}",
                fk.references_table,
                fk_value,
                table.name,
                key_parts.join(":")
            );
            data_bus.write_value(rel_key.as_bytes(), b"");
            relationship_count += 1;
        }
    }

    Ok(relationship_count)
}

fn parse_value(expression: &Expr) -> String {
    match expression {
        Expr::Value(Value::Number(number, _)) => number.clone(),
        Expr::Value(Value::SingleQuotedString(text)) => text.clone(),
        Expr::Value(Value::DoubleQuotedString(text)) => text.clone(),
        Expr::Value(Value::Null) => String::new(),
        Expr::Value(Value::Boolean(boolean)) => boolean.to_string(),
        _ => format!("{expression}"),
    }
}

pub struct ImportResult {
    pub tables: usize,
    pub rows: usize,
    pub relationships: usize,
    pub errors: usize,
}

pub struct DiffResult {
    pub additions: usize,
    pub modifications: usize,
    pub deletions: usize,
}

pub fn parse_diff_import(
    old_path: &str,
    new_path: &str,
    data_bus: &Arc<dyn DataBus>,
) -> Result<DiffResult, String> {
    let old_entries = read_tric_archive(old_path)?;
    let new_entries = read_tric_archive(new_path)?;

    let mut result = DiffResult {
        additions: 0,
        modifications: 0,
        deletions: 0,
    };

    for (path, new_value) in &new_entries {
        if path == "_meta/version" {
            continue;
        }
        let key = parse_tar_path_to_key(path);
        match old_entries.get(path) {
            Some(old_value) if old_value == new_value => {}
            Some(_) => {
                write_diff_entry(&key, new_value, path, data_bus);
                result.modifications += 1;
            }
            None => {
                write_diff_entry(&key, new_value, path, data_bus);
                result.additions += 1;
            }
        }
    }

    for path in old_entries.keys() {
        if path == "_meta/version" {
            continue;
        }
        if !new_entries.contains_key(path) {
            let key = parse_tar_path_to_key(path);
            data_bus.delete_value(key.as_bytes());
            result.deletions += 1;
        }
    }

    Ok(result)
}

fn read_tric_archive(path: &str) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
    let try_brotli = brotli::Decompressor::new(
        std::io::BufReader::new(
            std::fs::File::open(path).map_err(|error| format!("cannot open {path}: {error}"))?,
        ),
        4096,
    );

    if let Ok(parsed) = read_tar_entries(try_brotli) {
        return Ok(parsed);
    }

    let file = std::fs::File::open(path).map_err(|error| format!("cannot open {path}: {error}"))?;

    if let Ok(parsed) = read_tar_entries(file) {
        return Ok(parsed);
    }

    Err(format!("cannot parse {path} as .tric archive"))
}

fn read_tar_entries<R: std::io::Read>(
    reader: R,
) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
    let mut archive = tar::Archive::new(reader);
    let mut entries = std::collections::HashMap::new();

    for entry in archive
        .entries()
        .map_err(|error| format!("tar read failed: {error}"))?
    {
        let mut entry = entry.map_err(|error| format!("tar entry failed: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("tar path failed: {error}"))?
            .to_string_lossy()
            .to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut data)
            .map_err(|error| format!("tar read failed: {error}"))?;
        entries.insert(path, data);
    }

    Ok(entries)
}

fn parse_tar_path_to_key(path: &str) -> String {
    if let Some(table_name) = path.strip_prefix("_schema/") {
        format!("_schema:{table_name}")
    } else if let Some(rel_name) = path.strip_prefix("_rel/") {
        format!("_rel:{rel_name}")
    } else if let Some(ttl_path) = path.strip_prefix("_ttl/") {
        let key = ttl_path.replacen('/', ":", 1);
        format!("_ttl:{key}")
    } else {
        path.replacen('/', ":", 1)
    }
}

fn write_diff_entry(key: &str, value: &[u8], path: &str, data_bus: &Arc<dyn DataBus>) {
    if path.starts_with("_ttl/") {
        let data_key = path
            .strip_prefix("_ttl/")
            .unwrap_or("")
            .replacen('/', ":", 1);
        if let Ok(ttl_str) = std::str::from_utf8(value) {
            if let Ok(ttl_ms) = ttl_str.trim().parse::<u64>() {
                if ttl_ms > 0 {
                    data_bus.write_ttl(
                        data_key.as_bytes(),
                        std::time::Duration::from_millis(ttl_ms),
                    );
                }
            }
        }
    } else {
        data_bus.write_value(key.as_bytes(), value);
    }
}
