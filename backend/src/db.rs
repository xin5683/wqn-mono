#![allow(dead_code)]

use serde_json::Value;
use sqlx::{PgPool, Row, postgres::PgRow, types::Json};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub fn parse_uuid(value: &str, label: &str) -> AppResult<Uuid> {
    Uuid::parse_str(value).map_err(|_| AppError::BadRequest(format!("Invalid {label} format")))
}

pub fn row_json(row: PgRow) -> AppResult<Value> {
    row.try_get::<Json<Value>, _>("data")
        .map(|Json(value)| value)
        .map_err(AppError::from)
}

pub async fn fetch_json(pool: &PgPool, sql: &str) -> AppResult<Vec<Value>> {
    let rows = sqlx::query(sql).fetch_all(pool).await?;
    rows.into_iter().map(row_json).collect()
}

pub async fn fetch_one_json(pool: &PgPool, sql: &str) -> AppResult<Value> {
    let row = sqlx::query(sql).fetch_optional(pool).await?;
    row.map(row_json)
        .transpose()?
        .ok_or_else(|| AppError::NotFound("Not found".to_owned()))
}

pub fn normalise_json_object(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        map.retain(|_, v| !v.is_null());
    }
    value
}

pub fn value_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}

pub fn value_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub fn json_array(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub fn uuid_array(value: &Value, key: &str) -> AppResult<Vec<Uuid>> {
    json_array(value, key)
        .into_iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| AppError::BadRequest(format!("{key} must contain UUID strings")))
                .and_then(|s| parse_uuid(s, key))
        })
        .collect()
}
