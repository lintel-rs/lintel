use serde_json::{Map, Value};

/// Migrate boolean `exclusiveMinimum`/`exclusiveMaximum` (draft-04).
///
/// In draft-04, `exclusiveMinimum: true` means "use the companion `minimum`
/// value as the exclusive bound". In 2020-12, `exclusiveMinimum` is a
/// standalone numeric value.
pub fn migrate_numeric_bounds(obj: &mut Map<String, Value>) {
    migrate_bound(obj, "exclusiveMinimum", "minimum");
    migrate_bound(obj, "exclusiveMaximum", "maximum");
}

/// Migrate a single boolean exclusive bound.
///
/// - `true` + companion present: exclusive = companion value, companion removed
/// - `true` without companion: exclusive removed
/// - `false`: exclusive removed, companion unchanged
fn migrate_bound(obj: &mut Map<String, Value>, exclusive_key: &str, companion_key: &str) {
    let Some(&Value::Bool(b)) = obj.get(exclusive_key) else {
        return;
    };
    if b {
        if let Some(comp) = obj.remove(companion_key) {
            obj.insert(exclusive_key.to_string(), comp);
        } else {
            obj.remove(exclusive_key);
        }
    } else {
        obj.remove(exclusive_key);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(value: serde_json::Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn bool_true_promotes_companion() {
        let mut m = obj(json!({"minimum": 5, "exclusiveMinimum": true}));
        migrate_numeric_bounds(&mut m);
        assert_eq!(m["exclusiveMinimum"], 5);
        assert!(!m.contains_key("minimum"));
    }

    #[test]
    fn bool_false_removes_exclusive() {
        let mut m = obj(json!({"minimum": 5, "exclusiveMinimum": false}));
        migrate_numeric_bounds(&mut m);
        assert!(!m.contains_key("exclusiveMinimum"));
        assert_eq!(m["minimum"], 5);
    }

    #[test]
    fn numeric_value_unchanged() {
        let mut m = obj(json!({"exclusiveMinimum": 5}));
        migrate_numeric_bounds(&mut m);
        assert_eq!(m["exclusiveMinimum"], 5);
    }

    #[test]
    fn both_bounds_migrated() {
        let mut m = obj(json!({
            "minimum": 0, "exclusiveMinimum": true,
            "maximum": 100, "exclusiveMaximum": true
        }));
        migrate_numeric_bounds(&mut m);
        assert_eq!(m["exclusiveMinimum"], 0);
        assert_eq!(m["exclusiveMaximum"], 100);
        assert!(!m.contains_key("minimum"));
        assert!(!m.contains_key("maximum"));
    }
}
