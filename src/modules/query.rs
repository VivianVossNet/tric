// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Query — transforms SQL-subset strings into native TRIC+ KV operations.

use std::sync::Arc;

use sqlparser::ast::{BinaryOperator, Expr, FromTable, SetExpr, Statement, TableFactor, Value};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use crate::core::data_bus::DataBus;
use crate::modules::codec::Response;

const OK: u8 = 0x80;
const OK_PAYLOAD: u8 = 0x81;
const SCAN_CHUNK: u8 = 0x90;
const SCAN_END: u8 = 0x91;
const ERROR_MALFORMED: u8 = 0xA1;

pub fn parse_query(sql: &str, request_id: u32, data_bus: &Arc<dyn DataBus>) -> Vec<Response> {
    let statements = match Parser::parse_sql(&GenericDialect {}, sql) {
        Ok(statements) => statements,
        Err(_) => return vec![create_error(request_id, ERROR_MALFORMED, "invalid SQL")],
    };

    let Some(statement) = statements.into_iter().next() else {
        return vec![create_error(request_id, ERROR_MALFORMED, "empty SQL")];
    };

    match statement {
        Statement::Query(query) => parse_select(request_id, &query, data_bus),
        Statement::Insert(insert) => parse_insert(request_id, &insert, data_bus),
        Statement::Update {
            table,
            assignments,
            selection,
            ..
        } => parse_update(request_id, &table, &assignments, &selection, data_bus),
        Statement::Delete(delete) => parse_delete(request_id, &delete, data_bus),
        _ => vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "unsupported SQL statement",
        )],
    }
}

fn parse_select(
    request_id: u32,
    query: &sqlparser::ast::Query,
    data_bus: &Arc<dyn DataBus>,
) -> Vec<Response> {
    let SetExpr::Select(select) = query.body.as_ref() else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "unsupported query form",
        )];
    };

    let Some(table_name) = parse_table_name(&select.from) else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "missing FROM table",
        )];
    };

    match parse_where_condition(&select.selection) {
        WhereCondition::KeyEquals(key_value) => {
            let full_key = format!("{table_name}:{key_value}");
            match data_bus.read_value(full_key.as_bytes()) {
                Some(value) => {
                    let mut payload = Vec::with_capacity(4 + value.len());
                    payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
                    payload.extend_from_slice(&value);
                    vec![Response {
                        request_id,
                        opcode: OK_PAYLOAD,
                        payload,
                    }]
                }
                None => vec![Response {
                    request_id,
                    opcode: OK,
                    payload: Vec::new(),
                }],
            }
        }
        WhereCondition::KeyLikePrefix(prefix) => {
            let full_prefix = format!("{table_name}:{prefix}");
            let pairs = data_bus.find_by_prefix(full_prefix.as_bytes());
            let total = pairs.len().min(u16::MAX as usize) as u16;
            let mut responses = Vec::with_capacity(pairs.len() + 1);
            for (chunk_id, (key, value)) in pairs.iter().take(u16::MAX as usize).enumerate() {
                let mut payload = Vec::with_capacity(4 + key.len() + 4 + value.len() + 4);
                payload.extend_from_slice(&total.to_be_bytes());
                payload.extend_from_slice(&(chunk_id as u16).to_be_bytes());
                payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
                payload.extend_from_slice(key);
                payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
                payload.extend_from_slice(value);
                responses.push(Response {
                    request_id,
                    opcode: SCAN_CHUNK,
                    payload,
                });
            }
            responses.push(Response {
                request_id,
                opcode: SCAN_END,
                payload: Vec::new(),
            });
            responses
        }
        WhereCondition::None => {
            let full_prefix = format!("{table_name}:");
            let pairs = data_bus.find_by_prefix(full_prefix.as_bytes());
            let total = pairs.len().min(u16::MAX as usize) as u16;
            let mut responses = Vec::with_capacity(pairs.len() + 1);
            for (chunk_id, (key, value)) in pairs.iter().take(u16::MAX as usize).enumerate() {
                let mut payload = Vec::with_capacity(4 + key.len() + 4 + value.len() + 4);
                payload.extend_from_slice(&total.to_be_bytes());
                payload.extend_from_slice(&(chunk_id as u16).to_be_bytes());
                payload.extend_from_slice(&(key.len() as u32).to_be_bytes());
                payload.extend_from_slice(key);
                payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
                payload.extend_from_slice(value);
                responses.push(Response {
                    request_id,
                    opcode: SCAN_CHUNK,
                    payload,
                });
            }
            responses.push(Response {
                request_id,
                opcode: SCAN_END,
                payload: Vec::new(),
            });
            responses
        }
        WhereCondition::Unsupported => {
            vec![create_error(
                request_id,
                ERROR_MALFORMED,
                "unsupported WHERE condition",
            )]
        }
    }
}

fn parse_insert(
    request_id: u32,
    insert: &sqlparser::ast::Insert,
    data_bus: &Arc<dyn DataBus>,
) -> Vec<Response> {
    let table_name = insert.table_name.to_string().replace(['`', '"'], "");

    let schema_key = format!("_schema:{table_name}");
    let schema_data = data_bus.read_value(schema_key.as_bytes());

    let Some(source) = &insert.source else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "INSERT without VALUES",
        )];
    };
    let SetExpr::Values(values) = source.body.as_ref() else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "INSERT without VALUES",
        )];
    };

    for row in &values.rows {
        if row.is_empty() {
            continue;
        }
        let pk_value = parse_expr_value(&row[0]);
        let key = format!("{table_name}:{pk_value}");
        let value_parts: Vec<String> = row.iter().skip(1).map(parse_expr_value).collect();
        let value = value_parts.join("\n");
        data_bus.write_value(key.as_bytes(), value.as_bytes());
    }

    if schema_data.is_none() && !values.rows.is_empty() {
        let col_count = values.rows[0].len();
        let mut schema = "col0:INT:pk\n".to_string();
        for index in 1..col_count {
            schema.push_str(&format!("col{index}:TEXT\n"));
        }
        data_bus.write_value(schema_key.as_bytes(), schema.as_bytes());
    }

    vec![Response {
        request_id,
        opcode: OK,
        payload: Vec::new(),
    }]
}

fn parse_update(
    request_id: u32,
    table: &sqlparser::ast::TableWithJoins,
    assignments: &[sqlparser::ast::Assignment],
    selection: &Option<Expr>,
    data_bus: &Arc<dyn DataBus>,
) -> Vec<Response> {
    let Some(table_name) = parse_single_table_name(table) else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "unsupported UPDATE table",
        )];
    };

    let WhereCondition::KeyEquals(key_value) = parse_where_condition(selection) else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "UPDATE requires WHERE key = ...",
        )];
    };

    let full_key = format!("{table_name}:{key_value}");
    let current = data_bus.read_value(full_key.as_bytes());

    if current.is_some() && !assignments.is_empty() {
        let new_value: Vec<String> = assignments
            .iter()
            .map(|assignment| parse_expr_value(&assignment.value))
            .collect();
        data_bus.write_value(full_key.as_bytes(), new_value.join("\n").as_bytes());
    }

    vec![Response {
        request_id,
        opcode: OK,
        payload: Vec::new(),
    }]
}

fn parse_delete(
    request_id: u32,
    delete: &sqlparser::ast::Delete,
    data_bus: &Arc<dyn DataBus>,
) -> Vec<Response> {
    let tables = match &delete.from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };

    let Some(table_ref) = tables.first() else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "DELETE without FROM",
        )];
    };

    let table_name = match &table_ref.relation {
        TableFactor::Table { name, .. } => name.to_string().replace(['`', '"'], ""),
        _ => {
            return vec![create_error(
                request_id,
                ERROR_MALFORMED,
                "unsupported DELETE table",
            )]
        }
    };

    let WhereCondition::KeyEquals(key_value) = parse_where_condition(&delete.selection) else {
        return vec![create_error(
            request_id,
            ERROR_MALFORMED,
            "DELETE requires WHERE key = ...",
        )];
    };

    let full_key = format!("{table_name}:{key_value}");
    data_bus.delete_value(full_key.as_bytes());

    vec![Response {
        request_id,
        opcode: OK,
        payload: Vec::new(),
    }]
}

enum WhereCondition {
    KeyEquals(String),
    KeyLikePrefix(String),
    None,
    Unsupported,
}

fn parse_where_condition(selection: &Option<Expr>) -> WhereCondition {
    let Some(expr) = selection else {
        return WhereCondition::None;
    };

    match expr {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Eq,
            right,
        } => {
            if check_key_column(left) {
                if let Some(value) = parse_string_value(right) {
                    return WhereCondition::KeyEquals(value);
                }
            }
            WhereCondition::Unsupported
        }
        Expr::Like {
            expr: left,
            pattern,
            ..
        } => {
            if check_key_column(left) {
                if let Some(pattern_str) = parse_string_value(pattern) {
                    if let Some(prefix) = pattern_str.strip_suffix('%') {
                        return WhereCondition::KeyLikePrefix(prefix.to_string());
                    }
                }
            }
            WhereCondition::Unsupported
        }
        _ => WhereCondition::Unsupported,
    }
}

fn check_key_column(expr: &Expr) -> bool {
    matches!(expr,
        Expr::Identifier(ident) if ident.value.to_lowercase() == "key"
    )
}

fn parse_string_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Value(Value::SingleQuotedString(text)) => Some(text.clone()),
        Expr::Value(Value::Number(number, _)) => Some(number.clone()),
        _ => None,
    }
}

fn parse_expr_value(expr: &Expr) -> String {
    match expr {
        Expr::Value(Value::Number(number, _)) => number.clone(),
        Expr::Value(Value::SingleQuotedString(text)) => text.clone(),
        Expr::Value(Value::DoubleQuotedString(text)) => text.clone(),
        Expr::Value(Value::Null) => String::new(),
        Expr::Value(Value::Boolean(boolean)) => boolean.to_string(),
        _ => format!("{expr}"),
    }
}

fn parse_table_name(from: &[sqlparser::ast::TableWithJoins]) -> Option<String> {
    from.first().and_then(parse_single_table_name)
}

fn parse_single_table_name(table: &sqlparser::ast::TableWithJoins) -> Option<String> {
    match &table.relation {
        TableFactor::Table { name, .. } => Some(name.to_string().replace(['`', '"'], "")),
        _ => None,
    }
}

fn create_error(request_id: u32, opcode: u8, message: &str) -> Response {
    Response {
        request_id,
        opcode,
        payload: message.as_bytes().to_vec(),
    }
}
