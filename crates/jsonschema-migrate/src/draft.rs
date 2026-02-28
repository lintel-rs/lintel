/// Source JSON Schema draft, detected from the root `$schema` keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Draft {
    /// `http://json-schema.org/draft-04/schema#`
    Draft04,
    /// `http://json-schema.org/draft-06/schema#`
    Draft06,
    /// `http://json-schema.org/draft-07/schema#`
    Draft07,
    /// `https://json-schema.org/draft/2019-09/schema`
    Draft2019_09,
    /// `https://json-schema.org/draft/2020-12/schema`
    Draft2020_12,
}

/// Detect the JSON Schema draft from the root `$schema` keyword.
pub fn detect_draft(schema: &serde_json::Value) -> Option<Draft> {
    let s = schema.get("$schema")?.as_str()?;
    if s.contains("draft-04") {
        Some(Draft::Draft04)
    } else if s.contains("draft-06") {
        Some(Draft::Draft06)
    } else if s.contains("draft-07") {
        Some(Draft::Draft07)
    } else if s.contains("2019-09") {
        Some(Draft::Draft2019_09)
    } else if s.contains("2020-12") {
        Some(Draft::Draft2020_12)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn draft_04() {
        let schema = json!({"$schema": "http://json-schema.org/draft-04/schema#"});
        assert_eq!(detect_draft(&schema), Some(Draft::Draft04));
    }

    #[test]
    fn draft_06() {
        let schema = json!({"$schema": "http://json-schema.org/draft-06/schema#"});
        assert_eq!(detect_draft(&schema), Some(Draft::Draft06));
    }

    #[test]
    fn draft_07() {
        let schema = json!({"$schema": "http://json-schema.org/draft-07/schema#"});
        assert_eq!(detect_draft(&schema), Some(Draft::Draft07));
    }

    #[test]
    fn draft_2019_09() {
        let schema = json!({"$schema": "https://json-schema.org/draft/2019-09/schema"});
        assert_eq!(detect_draft(&schema), Some(Draft::Draft2019_09));
    }

    #[test]
    fn draft_2020_12() {
        let schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema"});
        assert_eq!(detect_draft(&schema), Some(Draft::Draft2020_12));
    }

    #[test]
    fn unknown() {
        let schema = json!({"$schema": "https://example.com/custom-schema"});
        assert_eq!(detect_draft(&schema), None);
    }

    #[test]
    fn missing() {
        let schema = json!({"type": "object"});
        assert_eq!(detect_draft(&schema), None);
    }
}
