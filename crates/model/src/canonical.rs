//! Canonical JSON for hashing and byte-for-byte determinism checks.
//!
//! Rules: object keys sorted by UTF-8 bytes regardless of insertion order; no insignificant
//! whitespace; integers verbatim; floats in the RFC 8785 / ECMAScript number format built from
//! the shortest round-trip digits of the standard library (so the output does not change when
//! serde_json swaps its float formatter); `-0` is written as `0`; strings use serde_json escaping.

use std::fmt::Write as _;

use serde::Serialize;
use serde_json::{Number, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
#[error("cannot canonicalize value: {0}")]
pub struct CanonicalError(String);

pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, &mut out);
    out
}

pub fn to_canonical_json<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    let value = serde_json::to_value(value).map_err(|e| CanonicalError(e.to_string()))?;
    Ok(canonical_json(&value))
}

/// Lowercase hex SHA-256 of the canonical form.
pub fn content_hash<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    let canonical = to_canonical_json(value)?;
    let digest = Sha256::digest(canonical.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

fn write_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => write_number(n, out),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(item, out);
            }
            out.push('}');
        }
    }
}

fn write_number(n: &Number, out: &mut String) {
    if n.is_i64() || n.is_u64() {
        out.push_str(&n.to_string());
        return;
    }
    match n.as_f64() {
        Some(f) if f.is_finite() => out.push_str(&format_f64(f)),
        _ => out.push_str(&n.to_string()),
    }
}

/// ECMAScript `Number::toString` layout over Rust's shortest round-trip digits.
fn format_f64(f: f64) -> String {
    if f == 0.0 {
        return "0".to_owned();
    }
    let sci = format!("{:e}", f.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits = mantissa.replace('.', "");
    let k = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    let n = exponent + 1;
    let zeros = |count: i32| "0".repeat(usize::try_from(count).unwrap_or(0));
    let split = |at: i32| digits.split_at(usize::try_from(at).unwrap_or(0));

    let body = if k <= n && n <= 21 {
        format!("{digits}{}", zeros(n - k))
    } else if 0 < n && n <= 21 {
        let (int, frac) = split(n);
        format!("{int}.{frac}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", zeros(-n))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        let (first, rest) = split(1);
        if rest.is_empty() {
            format!("{first}e{sign}{}", e.abs())
        } else {
            format!("{first}.{rest}e{sign}{}", e.abs())
        }
    };
    if f < 0.0 {
        format!("-{body}")
    } else {
        body
    }
}

fn write_string(s: &str, out: &mut String) {
    // serde_json string escaping is deterministic and cannot fail for &str.
    out.push_str(&serde_json::to_string(s).unwrap_or_default());
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    #[test]
    fn sorts_keys_and_strips_whitespace() {
        let v: Value =
            serde_json::from_str(r#"{ "b": 1, "a": { "d": [1, 2], "c": null } }"#).unwrap();
        assert_eq!(canonical_json(&v), r#"{"a":{"c":null,"d":[1,2]},"b":1}"#);
    }

    #[test]
    fn key_order_does_not_change_hash() {
        let a: Value = serde_json::from_str(r#"{"x":1,"y":{"p":"q","r":[true]}}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"y":{"r":[true],"p":"q"},"x":1}"#).unwrap();
        assert_eq!(content_hash(&a).unwrap(), content_hash(&b).unwrap());
        assert_ne!(
            content_hash(&a).unwrap(),
            content_hash(&json!({"x": 2})).unwrap()
        );
    }

    #[test]
    fn normalizes_numbers() {
        assert_eq!(
            canonical_json(&json!([1.0, -0.0, 2.5, -3, 1e300])),
            "[1,0,2.5,-3,1e+300]"
        );
        assert_eq!(canonical_json(&json!(u64::MAX)), "18446744073709551615");
    }

    #[test]
    fn floats_follow_ecmascript_layout() {
        let cases = [
            (1.0e16, "10000000000000000"),
            (1.0e21, "1e+21"),
            (123.456, "123.456"),
            (0.000001, "0.000001"),
            (0.0000001, "1e-7"),
            (-1.5e-9, "-1.5e-9"),
            (4.35, "4.35"),
            (0.1, "0.1"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
        ];
        for (input, expected) in cases {
            assert_eq!(canonical_json(&json!(input)), expected, "{input:e}");
        }
    }

    #[test]
    fn escapes_strings_like_serde_json() {
        assert_eq!(canonical_json(&json!("a\"b\n订单")), "\"a\\\"b\\n订单\"");
    }

    #[test]
    fn hash_is_lowercase_hex_sha256() {
        let h = content_hash(&json!({})).unwrap();
        assert_eq!(
            h,
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }

    fn arb_json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(|i| json!(i)),
            (-1.0e6f64..1.0e6).prop_map(|f| json!(f)),
            "[a-z0-9 \"\\\\]{0,8}".prop_map(Value::String),
        ];
        leaf.prop_recursive(4, 32, 6, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                prop::collection::btree_map("[a-z]{1,4}", inner, 0..6)
                    .prop_map(|m| Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn canonical_form_is_a_fixed_point(v in arb_json()) {
            let once = canonical_json(&v);
            let reparsed: Value = serde_json::from_str(&once).unwrap();
            prop_assert_eq!(canonical_json(&reparsed), once);
        }

        #[test]
        fn reversed_insertion_order_hashes_equal(
            entries in prop::collection::btree_map("[a-z]{1,4}", arb_json(), 0..8)
        ) {
            let forward: serde_json::Map<String, Value> = entries.clone().into_iter().collect();
            let backward: serde_json::Map<String, Value> = entries.into_iter().rev().collect();
            prop_assert_eq!(
                content_hash(&Value::Object(forward)).unwrap(),
                content_hash(&Value::Object(backward)).unwrap()
            );
        }
    }
}
