// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Import — reads SQL dump, applies Storage Plan, bulk-writes data and schema to DataBus.

use std::sync::Arc;

use sqlparser::ast::{Expr, SetExpr, Statement, Value};
use sqlparser::dialect::{GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;

use crate::core::data_bus::DataBus;
use crate::modules::analyser::{format_schema_entry, StoragePlan, TablePlan};

pub fn parse_sql(content: &str, format: &str) -> Vec<Statement> {
    let dialect: Box<dyn sqlparser::dialect::Dialect> = match format {
        "mysql" => Box::new(MySqlDialect {}),
        "postgres" => Box::new(PostgreSqlDialect {}),
        "sqlite" => Box::new(SQLiteDialect {}),
        _ => Box::new(GenericDialect {}),
    };
    Parser::parse_sql(&*dialect, content).unwrap_or_default()
}

pub fn execute_import(
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
        let schema_value = format_schema_entry(table);
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
            Some(extract_value(&values[index]))
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
        value_parts.push(extract_value(&values[index]));
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
            let fk_value = extract_value(&values[fk_index]);
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

fn extract_value(expression: &Expr) -> String {
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
