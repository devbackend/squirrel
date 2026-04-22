use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use uuid::Uuid;
use tokio_postgres::{types::Type, NoTls};

use crate::models::{ConnectionConfig, QueryResult};

pub async fn test_connection(cfg: &ConnectionConfig) -> Result<()> {
    let (client, connection) = tokio_postgres::connect(&cfg.connection_string(), NoTls)
        .await
        .context("connecting to postgres")?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });
    client.query("SELECT 1", &[]).await.context("test query")?;
    Ok(())
}

pub async fn execute_query(cfg: &ConnectionConfig, sql: &str) -> Result<QueryResult> {
    let (client, connection) = tokio_postgres::connect(&cfg.connection_string(), NoTls)
        .await
        .context("connecting to postgres")?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {e}");
        }
    });

    let sql_upper = sql.trim().to_ascii_uppercase();
    let is_query = sql_upper.starts_with("SELECT")
        || sql_upper.starts_with("WITH")
        || sql_upper.starts_with("TABLE")
        || sql_upper.starts_with("VALUES")
        || sql_upper.starts_with("SHOW")
        || sql_upper.starts_with("EXPLAIN");

    if is_query {
        let rows = client.query(sql, &[]).await.context("executing query")?;

        if rows.is_empty() {
            return Ok(QueryResult::Rows {
                columns: vec![],
                rows: vec![],
                page: 0,
                page_size: QueryResult::PAGE_SIZE,
                selected_row: 0,
            });
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        let data_rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| (0..row.len()).map(|i| row_value_to_string(row, i)).collect())
            .collect();

        Ok(QueryResult::Rows {
            columns,
            rows: data_rows,
            page: 0,
            page_size: QueryResult::PAGE_SIZE,
            selected_row: 0,
        })
    } else {
        let n = client.execute(sql, &[]).await.context("executing statement")?;
        Ok(QueryResult::AffectedRows(n))
    }
}

fn row_value_to_string(row: &tokio_postgres::Row, col_idx: usize) -> String {
    let t = row.columns()[col_idx].type_();

    macro_rules! try_as {
        ($rust_type:ty) => {
            if let Ok(val) = row.try_get::<_, Option<$rust_type>>(col_idx) {
                return val.map_or_else(|| "NULL".to_string(), |v| v.to_string());
            }
        };
    }

    if t == &Type::BOOL {
        try_as!(bool);
    } else if t == &Type::INT2 {
        try_as!(i16);
    } else if t == &Type::INT4 {
        try_as!(i32);
    } else if t == &Type::INT8 {
        try_as!(i64);
    } else if t == &Type::FLOAT4 {
        try_as!(f32);
    } else if t == &Type::FLOAT8 {
        try_as!(f64);
    } else if t == &Type::NUMERIC {
        try_as!(Decimal);
    } else if t == &Type::DATE {
        try_as!(NaiveDate);
    } else if t == &Type::TIMESTAMP {
        try_as!(NaiveDateTime);
    } else if t == &Type::TIMESTAMPTZ {
        try_as!(DateTime<Utc>);
    } else if t == &Type::TIME {
        try_as!(NaiveTime);
    } else if t == &Type::UUID {
        try_as!(Uuid);
    } else if (t == &Type::JSON || t == &Type::JSONB)
        && let Ok(val) = row.try_get::<_, Option<JsonValue>>(col_idx)
    {
        return val.map_or_else(|| "NULL".to_string(), |v| v.to_string());
    }

    // Default: String — covers TEXT, VARCHAR, BPCHAR, NAME, and other text-representable types.
    row.try_get::<_, Option<String>>(col_idx)
        .ok()
        .flatten()
        .unwrap_or_else(|| "NULL".to_string())
}
