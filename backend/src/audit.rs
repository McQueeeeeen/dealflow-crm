use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::AppResult;
use crate::models::{AuditAction, EntityType};

const IGNORED_FIELDS: [&str; 3] = ["updated_at", "version", "created_at"];

pub async fn record(
    conn: &mut PgConnection,
    actor: &AuthUser,
    entity: EntityType,
    entity_id: Uuid,
    action: AuditAction,
    changes: Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO audit_log (organization_id, actor_id, entity_type, entity_id, action, changes)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(actor.org_id)
    .bind(actor.user_id)
    .bind(entity.as_str())
    .bind(entity_id)
    .bind(action)
    .bind(changes)
    .execute(conn)
    .await?;
    Ok(())
}

pub fn snapshot<T: Serialize>(value: &T) -> Value {
    let mut value = serde_json::to_value(value).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        for field in IGNORED_FIELDS {
            map.remove(field);
        }
    }
    value
}

pub fn diff<T: Serialize>(before: &T, after: &T) -> Value {
    let (Value::Object(before), Value::Object(after)) = (snapshot(before), snapshot(after)) else {
        return Value::Object(Map::new());
    };
    let changes = after
        .iter()
        .filter(|(key, new)| before.get(*key) != Some(*new))
        .map(|(key, new)| {
            let old = before.get(key).cloned().unwrap_or(Value::Null);
            (key.clone(), serde_json::json!({ "from": old, "to": new }))
        })
        .collect();
    Value::Object(changes)
}

pub fn is_empty_diff(changes: &Value) -> bool {
    changes.as_object().is_none_or(Map::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Serialize)]
    struct Deal {
        title: &'static str,
        value_cents: i64,
        version: i32,
        updated_at: &'static str,
    }

    #[test]
    fn diff_reports_only_changed_fields() {
        let before = Deal {
            title: "Renewal",
            value_cents: 100,
            version: 1,
            updated_at: "a",
        };
        let after = Deal {
            title: "Renewal",
            value_cents: 250,
            version: 2,
            updated_at: "b",
        };
        assert_eq!(
            diff(&before, &after),
            json!({ "value_cents": { "from": 100, "to": 250 } })
        );
    }

    #[test]
    fn identical_snapshots_produce_empty_diff() {
        let a = Deal {
            title: "X",
            value_cents: 1,
            version: 1,
            updated_at: "a",
        };
        let b = Deal {
            title: "X",
            value_cents: 1,
            version: 7,
            updated_at: "z",
        };
        assert!(is_empty_diff(&diff(&a, &b)));
    }

    #[test]
    fn snapshot_strips_bookkeeping_fields() {
        let snap = snapshot(&Deal {
            title: "X",
            value_cents: 1,
            version: 3,
            updated_at: "a",
        });
        assert_eq!(snap, json!({ "title": "X", "value_cents": 1 }));
    }
}
