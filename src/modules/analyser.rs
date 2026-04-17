// Copyright 2025 Vivian Voss. Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0
// Scope: Analyser — transforms SQL AST into TRIC+ Storage Plan via seven deterministic rules.

use sqlparser::ast::{
    ColumnDef, ColumnOption, ColumnOptionDef, DataType, Statement, TableConstraint,
};

pub struct StoragePlan {
    pub tables: Vec<TablePlan>,
}

pub struct TablePlan {
    pub name: String,
    pub columns: Vec<ColumnPlan>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKeyPlan>,
    pub ttl_column: Option<String>,
}

pub struct ColumnPlan {
    pub name: String,
    pub data_type: String,
    pub is_primary_key: bool,
}

pub struct ForeignKeyPlan {
    pub column: String,
    pub references_table: String,
    pub references_column: String,
}

pub fn analyse_statements(statements: &[Statement]) -> StoragePlan {
    let mut tables = Vec::new();
    for statement in statements {
        if let Statement::CreateTable(create_table) = statement {
            if let Some(plan) = analyse_create_table(
                &create_table.name.to_string(),
                &create_table.columns,
                &create_table.constraints,
            ) {
                tables.push(plan);
            }
        }
    }
    StoragePlan { tables }
}

fn analyse_create_table(
    name: &str,
    columns: &[ColumnDef],
    constraints: &[TableConstraint],
) -> Option<TablePlan> {
    let sanitised_name = sanitise_table_name(name);
    if sanitised_name.is_empty() {
        return None;
    }

    let primary_key_columns = find_primary_key_columns(columns, constraints);
    let foreign_keys = find_foreign_keys(constraints);
    let ttl_column = find_ttl_candidate(columns);

    let column_plans: Vec<ColumnPlan> = columns
        .iter()
        .map(|column| ColumnPlan {
            name: column.name.value.clone(),
            data_type: format_data_type(&column.data_type),
            is_primary_key: primary_key_columns.contains(&column.name.value),
        })
        .collect();

    Some(TablePlan {
        name: sanitised_name,
        columns: column_plans,
        primary_key: primary_key_columns,
        foreign_keys,
        ttl_column,
    })
}

fn find_primary_key_columns(columns: &[ColumnDef], constraints: &[TableConstraint]) -> Vec<String> {
    for constraint in constraints {
        if let TableConstraint::PrimaryKey {
            columns: pk_cols, ..
        } = constraint
        {
            return pk_cols.iter().map(|column| column.value.clone()).collect();
        }
    }
    for column in columns {
        for option in &column.options {
            if matches!(
                option,
                ColumnOptionDef {
                    option: ColumnOption::Unique {
                        is_primary: true,
                        ..
                    },
                    ..
                }
            ) {
                return vec![column.name.value.clone()];
            }
        }
    }
    Vec::new()
}

fn find_foreign_keys(constraints: &[TableConstraint]) -> Vec<ForeignKeyPlan> {
    let mut foreign_keys = Vec::new();
    for constraint in constraints {
        if let TableConstraint::ForeignKey {
            columns,
            foreign_table,
            referred_columns,
            ..
        } = constraint
        {
            if let (Some(column), Some(ref_column)) = (columns.first(), referred_columns.first()) {
                foreign_keys.push(ForeignKeyPlan {
                    column: column.value.clone(),
                    references_table: foreign_table.to_string(),
                    references_column: ref_column.value.clone(),
                });
            }
        }
    }
    foreign_keys
}

fn find_ttl_candidate(columns: &[ColumnDef]) -> Option<String> {
    let ttl_names = [
        "expires_at",
        "expire_time",
        "valid_until",
        "ttl",
        "last_seen",
        "expiry",
    ];
    for column in columns {
        let lower_name = column.name.value.to_lowercase();
        if ttl_names.contains(&lower_name.as_str()) {
            return Some(column.name.value.clone());
        }
    }
    None
}

fn format_data_type(data_type: &DataType) -> String {
    format!("{data_type}")
}

fn sanitise_table_name(name: &str) -> String {
    name.replace(['`', '"'], "")
        .chars()
        .filter(|character| character.is_alphanumeric() || *character == '_')
        .collect()
}

pub fn format_schema_entry(table: &TablePlan) -> String {
    let mut schema = String::new();
    for column in &table.columns {
        let mut line = format!("{}:{}", column.name, column.data_type);
        if column.is_primary_key {
            line.push_str(":pk");
        }
        if let Some(fk) = table
            .foreign_keys
            .iter()
            .find(|fk| fk.column == column.name)
        {
            line.push_str(&format!(
                ":fk={}.{}",
                fk.references_table, fk.references_column
            ));
        }
        if table.ttl_column.as_deref() == Some(&column.name) {
            line.push_str(":ttl");
        }
        schema.push_str(&line);
        schema.push('\n');
    }
    schema
}

pub fn format_storage_plan(plan: &StoragePlan) -> String {
    let mut output = String::new();
    for table in &plan.tables {
        output.push_str(&format!(
            "{:<20} pk={:<15} ttl={:<15} fk={}\n",
            table.name,
            table.primary_key.join(","),
            table.ttl_column.as_deref().unwrap_or("—"),
            table
                .foreign_keys
                .iter()
                .map(|fk| format!("{}->{}", fk.column, fk.references_table))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    output
}
