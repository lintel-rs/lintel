use saphyr_parser::{Event, Parser};
fn main() {
    let inputs = vec![
        ("set.yml", "- 123\n  # 456\n"),
        ("in-empty-item", "a:\n  #123"),
        ("issue-10922", "foo: bar\n\n# End Comment\n"),
        (
            "map-4 simplified",
            "before:\n\n  # before.comment\nafter:\n",
        ),
        ("end-comment A:", "A:\n  B:\n #A\n   #A\n"),
        ("end-comment a:", "a:\n  b:\n   #b\n #a\n"),
        (
            "issue-9130",
            "- foo: 0\n  bar: 1\n\n  # baz: 2\n- quux: 3\n",
        ),
        (
            "map.yml",
            "foo1:\n  - foo: item1\n    bar: item1\n\n  # - foo: item2\n  #   bar: item2\n\n  # - foo: item3\n  #   bar: item3\n\n  - foo: item4\n    bar: item4\n",
        ),
        ("sequence.yml", "-  - a\n\n   # - b\n\n   # - c\n\n   - e\n"),
        (
            "empty-block-chomp",
            "strip: >-\n\nclip: >\n\nkeep: |+\n\n\n",
        ),
        (
            "spec-8-18",
            "plain key: in-line value\n: # Both empty\n\"quoted key\":\n- entry\n",
        ),
        (
            "spec-8-20",
            "- \"flow in block\"\n- >\n  Block scalar\n- !!map # Block collection\n  foo: bar\n",
        ),
        (
            "spec-6-19",
            "%TAG !! tag:example.com,2000:app/\n---\n!!int 1 - 3 # Interval, not integer\n",
        ),
        (
            "tags-on-empty",
            "- !!str\n- !!null : a\n  b: !!str\n- !!str : !!null\n",
        ),
    ];
    for (name, input) in inputs {
        println!("=== {} ===", name);
        println!("Input: {:?}", input);
        let parser = Parser::new_from_str(input);
        for result in parser {
            let (event, span) = result.unwrap();
            let evt_name = match &event {
                Event::StreamStart => "StreamStart".to_string(),
                Event::StreamEnd => "StreamEnd".to_string(),
                Event::DocumentStart(e) => format!("DocumentStart(explicit={})", e),
                Event::DocumentEnd => "DocumentEnd".to_string(),
                Event::MappingStart(a, t) => format!("MappingStart(anchor={}, tag={:?})", a, t),
                Event::MappingEnd => "MappingEnd".to_string(),
                Event::SequenceStart(a, t) => format!("SequenceStart(anchor={}, tag={:?})", a, t),
                Event::SequenceEnd => "SequenceEnd".to_string(),
                Event::Scalar(v, s, a, t) => {
                    format!("Scalar({:?}, {:?}, anchor={}, tag={:?})", v, s, a, t)
                }
                Event::Alias(id) => format!("Alias({})", id),
                _ => format!("{:?}", event),
            };
            println!(
                "  {} start={}:{} end={}:{}",
                evt_name,
                span.start.line(),
                span.start.col(),
                span.end.line(),
                span.end.col()
            );
        }
        println!();
    }
}

// Additional debug: format and compare
