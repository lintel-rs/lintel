use prettier_rs::{Format, PrettierOptions, format_str};
fn main() {
    // set.yml: "- 123\n  # 456\n"
    let input = "- 123\n  # 456\n";
    let opts = PrettierOptions::default();
    let r = format_str(input, Format::Yaml, &opts).unwrap();
    println!("Input:    {:?}", input);
    println!("Output:   {:?}", r);
    println!("Expected: {:?}", "- 123\n  # 456\n");

    // in-empty-item-without-newlline.yml: "a:\n  #123"
    let input2 = "a:\n  #123";
    let r2 = format_str(input2, Format::Yaml, &opts).unwrap();
    println!("\nInput:    {:?}", input2);
    println!("Output:   {:?}", r2);
    println!("Expected: {:?}", "a:\n  #123\n");

    // issue-10922.yml: "foo: bar\n\n# End Comment - Don't remove previous empty line\n"
    let input3 = "foo: bar\n\n# End Comment - Don't remove previous empty line\n";
    let r3 = format_str(input3, Format::Yaml, &opts).unwrap();
    println!("\nInput:    {:?}", input3);
    println!("Output:   {:?}", r3);
    println!(
        "Expected: {:?}",
        "foo: bar\n\n# End Comment - Don't remove previous empty line\n"
    );

    // map-4.yml simplified: "before:\n\n  # before.comment\nafter:\n  # after.comment\n"
    let input4 = "before:\n\n  # before.comment\nafter:\n  # after.comment\n";
    let r4 = format_str(input4, Format::Yaml, &opts).unwrap();
    println!("\nInput:    {:?}", input4);
    println!("Output:   {:?}", r4);
    println!(
        "Expected: {:?}",
        "before:\n\n  # before.comment\nafter:\n  # after.comment\n"
    );

    // sequence.yml
    let input_s = "-  - a\n\n   # - b\n\n   # - c\n\n   - e\n";
    let rs = format_str(input_s, Format::Yaml, &opts).unwrap();
    println!("\n=== sequence.yml ===");
    println!("Output:");
    for (i, line) in rs.lines().enumerate() {
        println!("  {}: {:?}", i, line);
    }
    let expected_s = "- - a\n\n  # - b\n\n  # - c\n\n  - e\n";
    println!("Expected:");
    for (i, line) in expected_s.lines().enumerate() {
        println!("  {}: {:?}", i, line);
    }
    println!("Match: {}", rs == expected_s);

    // issue-9130.yml
    let input5 = "- foo: 0\n  bar: 1\n\n  # baz: 2\n- quux: 3\n\n- foo: 0\n  bar: 1\n\n  # baz: 2\n\n  # baz: 3\n- quux: 3\n";
    let expected5 = input5; // idempotent
    let r5 = format_str(input5, Format::Yaml, &opts).unwrap();
    println!("\nInput:    {:?}", input5);
    println!("Output:   {:?}", r5);
    println!("Expected: {:?}", expected5);
    println!("Match: {}", r5 == expected5);
    if r5 != expected5 {
        println!("Output lines:");
        for (i, line) in r5.lines().enumerate() {
            println!("  {}: {:?}", i, line);
        }
        println!("Expected lines:");
        for (i, line) in expected5.lines().enumerate() {
            println!("  {}: {:?}", i, line);
        }
    }
}
