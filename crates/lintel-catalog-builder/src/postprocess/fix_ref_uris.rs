use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// Characters that must be percent-encoded in URI fragment components.
///
/// Per RFC 3986, fragments may contain: `pchar / "/" / "?"` where
/// `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`.
///
/// This set encodes everything that is NOT allowed in a fragment.
const FRAGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b']')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'\\')
    .add(b'^')
    .add(b'"')
    .add(b'`');

/// Percent-encode invalid characters in `$ref` URI references.
///
/// Many schemas in the wild use definition names with spaces, brackets, angle
/// brackets, etc. that are not valid in URI references per RFC 3986. This
/// function fixes them by percent-encoding the offending characters in the
/// fragment portion of `$ref` values.
pub(super) fn fix_ref_uris(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref")
                && let Some(new_ref) = encode_ref_fragment(ref_str)
            {
                map.insert("$ref".to_string(), serde_json::Value::String(new_ref));
            }
            for v in map.values_mut() {
                fix_ref_uris(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                fix_ref_uris(v);
            }
        }
        _ => {}
    }
}

/// Encode invalid characters in a `$ref` fragment. Returns `None` if no
/// encoding is needed.
fn encode_ref_fragment(ref_str: &str) -> Option<String> {
    let (base, fragment) = ref_str.split_once('#')?;

    // Encode each JSON Pointer segment individually, preserving `/` separators
    let encoded_fragment: String = fragment
        .split('/')
        .map(|segment| utf8_percent_encode(segment, FRAGMENT_ENCODE_SET).to_string())
        .collect::<Vec<_>>()
        .join("/");

    if encoded_fragment == fragment {
        return None;
    }

    Some(format!("{base}#{encoded_fragment}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_fragment_with_spaces() {
        assert_eq!(
            encode_ref_fragment("#/$defs/Parameter Node"),
            Some("#/$defs/Parameter%20Node".to_string())
        );
    }

    #[test]
    fn encode_fragment_with_brackets() {
        assert_eq!(
            encode_ref_fragment("#/$defs/ConfigTranslated[string]"),
            Some("#/$defs/ConfigTranslated%5Bstring%5D".to_string())
        );
    }

    #[test]
    fn encode_fragment_with_angle_brackets() {
        assert_eq!(
            encode_ref_fragment("#/definitions/Dictionary<any>"),
            Some("#/definitions/Dictionary%3Cany%3E".to_string())
        );
    }

    #[test]
    fn encode_fragment_with_pipe() {
        assert_eq!(
            encode_ref_fragment("#/definitions/k8s.io|api|core|v1.TaintEffect"),
            Some("#/definitions/k8s.io%7Capi%7Ccore%7Cv1.TaintEffect".to_string())
        );
    }

    #[test]
    fn encode_fragment_valid_unchanged() {
        assert_eq!(encode_ref_fragment("#/definitions/Foo"), None);
        assert_eq!(encode_ref_fragment("#/$defs/bar-baz"), None);
    }

    #[test]
    fn encode_no_fragment() {
        assert_eq!(encode_ref_fragment("https://example.com/foo.json"), None);
    }

    #[test]
    fn encodes_spaces_in_schema() {
        let mut schema = serde_json::json!({
            "oneOf": [
                { "$ref": "#/$defs/Parameter Node" },
                { "$ref": "#/$defs/Event Node" }
            ],
            "properties": {
                "ok": { "$ref": "#/definitions/Valid" }
            }
        });
        fix_ref_uris(&mut schema);
        assert_eq!(schema["oneOf"][0]["$ref"], "#/$defs/Parameter%20Node");
        assert_eq!(schema["oneOf"][1]["$ref"], "#/$defs/Event%20Node");
        assert_eq!(schema["properties"]["ok"]["$ref"], "#/definitions/Valid");
    }

    #[test]
    fn encodes_complex_rust_types() {
        let mut schema = serde_json::json!({
            "$ref": "#/definitions/core::option::Option<vector::template::Template>"
        });
        fix_ref_uris(&mut schema);
        assert_eq!(
            schema["$ref"],
            "#/definitions/core::option::Option%3Cvector::template::Template%3E"
        );
    }
}
