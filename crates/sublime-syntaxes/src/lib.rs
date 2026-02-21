use syntect::parsing::SyntaxSet;

const SYNTAX_SET_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/syntaxes.bin"));

/// Returns the precompiled [`SyntaxSet`] containing extra syntax definitions
/// not included in syntect's defaults (e.g. TOML).
///
/// The syntax set is deserialized from a binary blob that was compiled at build
/// time, so this is cheaper than parsing YAML definitions at runtime.
pub fn extra_syntax_set() -> SyntaxSet {
    syntect::dumps::from_binary(SYNTAX_SET_BINARY)
}
