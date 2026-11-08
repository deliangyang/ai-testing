//! Resolving a reference path against a JSON Schema, without any network access.

use model::PathSeg;
use serde_json::Value;

const MAX_REF_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// The path exists, or the schema is open at some point so it may exist.
    Found,
    /// `segment` (index into the path) cannot exist; `reason` explains why.
    Missing { segment: usize, reason: String },
}

/// Walks `path` through `schema`. `$ref`s are resolved as JSON Pointers into `schema` itself.
pub fn lookup(schema: &Value, path: &[PathSeg]) -> Lookup {
    walk(schema, schema, path, 0, 0)
}

fn walk(root: &Value, schema: &Value, path: &[PathSeg], at: usize, depth: usize) -> Lookup {
    if depth > MAX_REF_DEPTH {
        return Lookup::Found;
    }
    let Some(segment) = path.get(at) else {
        return Lookup::Found;
    };
    let Value::Object(obj) = schema else {
        // `true` or anything non-object: no constraints.
        return match schema {
            Value::Bool(false) => missing(at, "schema forbids any value here"),
            _ => Lookup::Found,
        };
    };

    if let Some(Value::String(reference)) = obj.get("$ref") {
        return match resolve_ref(root, reference) {
            Some(target) => walk(root, target, path, at, depth + 1),
            None => Lookup::Found,
        };
    }
    // A field exists under allOf if any branch that declares a shape has it.
    if let Some(Value::Array(branches)) = obj.get("allOf") {
        let mut first_missing = None;
        for branch in branches.iter().filter(|b| declares_shape(b)) {
            match walk(root, branch, path, at, depth + 1) {
                Lookup::Found => return Lookup::Found,
                missing => {
                    first_missing.get_or_insert(missing);
                }
            }
        }
        if let Some(missing) = first_missing {
            if own_shape_absent(obj) {
                return missing;
            }
        }
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(Value::Array(branches)) = obj.get(key) {
            let results: Vec<Lookup> = branches
                .iter()
                .map(|b| walk(root, b, path, at, depth + 1))
                .collect();
            return results
                .iter()
                .find(|r| **r == Lookup::Found)
                .cloned()
                .or_else(|| results.into_iter().next())
                .unwrap_or(Lookup::Found);
        }
    }

    let types = declared_types(obj);
    match segment {
        PathSeg::Key(key) => {
            if !types.is_empty() && !types.iter().any(|t| t == "object") {
                return missing(
                    at,
                    format!(
                        "'{key}' is a field access but the value is {}",
                        types.join("|")
                    ),
                );
            }
            if let Some(Value::Object(props)) = obj.get("properties") {
                if let Some(child) = props.get(key) {
                    return walk(root, child, path, at + 1, depth);
                }
            }
            match obj.get("additionalProperties") {
                Some(Value::Object(_)) => {
                    let child = &obj["additionalProperties"];
                    walk(root, child, path, at + 1, depth)
                }
                Some(Value::Bool(false)) => missing(at, format!("field '{key}' is not declared")),
                _ if obj.contains_key("properties") => {
                    missing(at, format!("field '{key}' is not declared"))
                }
                _ => Lookup::Found,
            }
        }
        PathSeg::Index(index) => {
            if !types.is_empty() && !types.iter().any(|t| t == "array") {
                return missing(
                    at,
                    format!("[{index}] is an index but the value is {}", types.join("|")),
                );
            }
            if let Some(Value::Array(prefix)) = obj.get("prefixItems") {
                if let Some(child) = prefix.get(*index) {
                    return walk(root, child, path, at + 1, depth);
                }
            }
            match obj.get("items") {
                Some(Value::Array(tuple)) => match tuple.get(*index) {
                    Some(child) => walk(root, child, path, at + 1, depth),
                    None => Lookup::Found,
                },
                Some(child) => walk(root, child, path, at + 1, depth),
                None => Lookup::Found,
            }
        }
    }
}

fn missing(segment: usize, reason: impl Into<String>) -> Lookup {
    Lookup::Missing {
        segment,
        reason: reason.into(),
    }
}

fn declared_types(obj: &serde_json::Map<String, Value>) -> Vec<String> {
    match obj.get("type") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(ts)) => ts
            .iter()
            .filter_map(|t| t.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn declares_shape(schema: &Value) -> bool {
    schema.as_object().is_some_and(|o| {
        [
            "properties",
            "additionalProperties",
            "items",
            "prefixItems",
            "$ref",
        ]
        .iter()
        .any(|k| o.contains_key(*k))
    })
}

fn own_shape_absent(obj: &serde_json::Map<String, Value>) -> bool {
    !["properties", "additionalProperties", "items", "prefixItems"]
        .iter()
        .any(|k| obj.contains_key(*k))
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    root.pointer(pointer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn keys(path: &[&str]) -> Vec<PathSeg> {
        path.iter()
            .map(|s| match s.parse::<usize>() {
                Ok(i) => PathSeg::Index(i),
                Err(_) => PathSeg::Key((*s).to_owned()),
            })
            .collect()
    }

    fn order_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {"type": "string"},
                "items": {"type": "array", "items": {"$ref": "#/$defs/Item"}},
                "meta": {"type": "object", "additionalProperties": {"type": "string"}},
                "extra": {},
                "pair": {"type": "array", "prefixItems": [{"type": "string"}, {"$ref": "#/$defs/Item"}]},
                "payer": {"oneOf": [
                    {"type": "object", "properties": {"card": {"type": "string"}}},
                    {"type": "object", "properties": {"wallet": {"type": "string"}}}
                ]},
                "base": {"allOf": [{"$ref": "#/$defs/Item"}, {"properties": {"note": {"type": "string"}}}]}
            },
            "$defs": {"Item": {"type": "object", "properties": {"sku": {"type": "string"}}}}
        })
    }

    #[test]
    fn existing_paths_are_found() {
        let schema = order_schema();
        for path in [
            vec!["id"],
            vec!["items", "0", "sku"],
            vec!["meta", "anything"],
            vec!["extra", "deep", "0", "x"],
            vec!["pair", "1", "sku"],
            vec!["payer", "wallet"],
            vec!["base", "sku"],
            vec!["base", "note"],
            vec![],
        ] {
            assert_eq!(lookup(&schema, &keys(&path)), Lookup::Found, "{path:?}");
        }
    }

    #[test]
    fn missing_paths_report_the_failing_segment() {
        let schema = order_schema();
        let cases = [
            (vec!["nope"], 0),
            (vec!["items", "0", "price"], 2),
            (vec!["id", "x"], 1),
            (vec!["items", "sku"], 1),
            (vec!["payer", "cash"], 1),
            (vec!["base", "missing"], 1),
        ];
        for (path, segment) in cases {
            match lookup(&schema, &keys(&path)) {
                Lookup::Missing { segment: s, .. } => assert_eq!(s, segment, "{path:?}"),
                Lookup::Found => panic!("expected missing for {path:?}"),
            }
        }
    }

    #[test]
    fn open_schemas_accept_anything() {
        for schema in [json!({}), json!(true), json!({"type": "object"})] {
            assert_eq!(
                lookup(&schema, &keys(&["a", "0", "b"])),
                Lookup::Found,
                "{schema}"
            );
        }
    }

    #[test]
    fn closed_object_without_properties_rejects_fields() {
        let schema = json!({"type": "object", "additionalProperties": false});
        assert!(matches!(
            lookup(&schema, &keys(&["a"])),
            Lookup::Missing { segment: 0, .. }
        ));
    }

    #[test]
    fn recursive_refs_terminate() {
        let schema = json!({"$ref": "#"});
        assert_eq!(lookup(&schema, &keys(&["a"])), Lookup::Found);
    }
}
