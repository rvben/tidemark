//! Output rendering: JSON envelopes and human tables, pagination, field selection.

use serde_json::Value;

/// The effective output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Table,
}

/// Resolve the effective format: explicit flag wins, else JSON when not a TTY.
pub fn resolve_format(explicit: Option<Format>, stdout_is_tty: bool) -> Format {
    match explicit {
        Some(f) => f,
        None => {
            if stdout_is_tty {
                Format::Table
            } else {
                Format::Json
            }
        }
    }
}

/// Apply offset/limit to a slice of JSON values, returning (page, total).
pub fn paginate(items: Vec<Value>, offset: usize, limit: Option<usize>) -> (Vec<Value>, usize) {
    let total = items.len();
    let mut it = items.into_iter().skip(offset);
    let page: Vec<Value> = match limit {
        Some(n) => it.by_ref().take(n).collect(),
        None => it.by_ref().collect(),
    };
    (page, total)
}

/// Keep only `fields` keys in each object (no-op if `fields` is empty).
pub fn select_fields(items: Vec<Value>, fields: &[String]) -> Vec<Value> {
    if fields.is_empty() {
        return items;
    }
    items
        .into_iter()
        .map(|v| {
            if let Value::Object(map) = v {
                let mut out = serde_json::Map::new();
                for f in fields {
                    if let Some(val) = map.get(f) {
                        out.insert(f.clone(), val.clone());
                    }
                }
                Value::Object(out)
            } else {
                v
            }
        })
        .collect()
}

/// Wrap a page into the clispec list envelope.
pub fn list_envelope(items: Vec<Value>, total: usize, limit: Option<usize>, offset: usize) -> Value {
    serde_json::json!({
        "items": items,
        "total": total,
        "limit": limit,
        "offset": offset
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn json_when_piped_table_when_tty() {
        assert_eq!(resolve_format(None, false), Format::Json);
        assert_eq!(resolve_format(None, true), Format::Table);
        assert_eq!(resolve_format(Some(Format::Json), true), Format::Json);
    }

    #[test]
    fn paginate_offset_and_limit() {
        let items: Vec<Value> = (0..5).map(|i| json!({ "n": i })).collect();
        let (page, total) = paginate(items, 1, Some(2));
        assert_eq!(total, 5);
        assert_eq!(page, vec![json!({"n":1}), json!({"n":2})]);
    }

    #[test]
    fn select_fields_keeps_only_requested() {
        let items = vec![json!({"a":1,"b":2,"c":3})];
        let got = select_fields(items, &["a".into(), "c".into()]);
        assert_eq!(got, vec![json!({"a":1,"c":3})]);
    }

    #[test]
    fn empty_fields_is_noop() {
        let items = vec![json!({"a":1})];
        let got = select_fields(items.clone(), &[]);
        assert_eq!(got, items);
    }

    #[test]
    fn list_envelope_has_required_shape() {
        let env = list_envelope(vec![json!({"a":1})], 3, Some(1), 0);
        assert_eq!(env["total"], 3);
        assert_eq!(env["limit"], 1);
        assert_eq!(env["offset"], 0);
        assert_eq!(env["items"].as_array().unwrap().len(), 1);
    }
}
