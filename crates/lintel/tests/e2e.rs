use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cases_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../e2e-tests/cases")
        .canonicalize()
        .expect("e2e-tests/cases directory must exist")
}

fn run_lintel_ci(case_name: &str) -> Output {
    let case_dir = cases_root().join(case_name);
    assert!(case_dir.is_dir(), "case directory not found: {case_name}");

    Command::new(env!("CARGO_BIN_EXE_lintel"))
        .args(["ci", "--no-catalog", "--no-cache"])
        .current_dir(&case_dir)
        .output()
        .expect("failed to execute lintel")
}

fn snapshot_output(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);
    format!("{stderr}exit code: {code}\n")
}

macro_rules! e2e_test {
    ($name:ident) => {
        #[test]
        #[ignore]
        fn $name() {
            let case = stringify!($name).replace('_', "-");
            let output = run_lintel_ci(&case);
            let text = snapshot_output(&output);

            let mut settings = insta::Settings::clone_current();
            settings.add_filter(r" in \d+ms", " in [TIME]");
            settings.bind(|| {
                insta::assert_snapshot!(text);
            });
        }
    };
}

e2e_test!(malformed_json);
e2e_test!(malformed_yaml);
e2e_test!(malformed_trailing_comma);
e2e_test!(malformed_package_json);
e2e_test!(multiple_errors);

e2e_test!(schemastore);
