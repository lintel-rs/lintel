#![allow(clippy::unwrap_used)]

use criterion::{Criterion, criterion_group, criterion_main};

const PATTERNS: &[&str] = &[
    "**/*.rs",
    "**/*.toml",
    "src/**/*.js",
    "*.md",
    "tests/**/*.test.ts",
    "docs/**/*.html",
    "{src,lib}/**/*.rs",
    "**/*.json",
];

const PATHS: &[&str] = &[
    "src/main.rs",
    "Cargo.toml",
    "src/components/button.js",
    "README.md",
    "tests/unit/foo.test.ts",
    "docs/api/index.html",
    "lib/core/parser.rs",
    "package.json",
    "src/index.css",
    "some/deep/nested/path/to/file.txt",
];

// -- is_match benchmarks --

fn bench_tiny_glob_set(c: &mut Criterion) {
    let mut builder = glob_set::TinyGlobSetBuilder::new();
    for pat in PATTERNS {
        builder.add(glob_set::Glob::new(pat).unwrap());
    }
    let set = builder.build().unwrap();

    c.bench_function("tiny_glob_set", |b| {
        b.iter(|| {
            for path in PATHS {
                set.is_match(*path);
            }
        });
    });
}

fn bench_glob_set(c: &mut Criterion) {
    let mut builder = glob_set::GlobSetBuilder::new();
    for pat in PATTERNS {
        builder.add(glob_set::Glob::new(pat).unwrap());
    }
    let set = builder.build().unwrap();

    c.bench_function("glob_set", |b| {
        b.iter(|| {
            for path in PATHS {
                set.is_match(*path);
            }
        });
    });
}

fn bench_globset(c: &mut Criterion) {
    let mut builder = globset::GlobSetBuilder::new();
    for pat in PATTERNS {
        builder.add(globset::Glob::new(pat).unwrap());
    }
    let set = builder.build().unwrap();

    c.bench_function("globset", |b| {
        b.iter(|| {
            for path in PATHS {
                set.is_match(*path);
            }
        });
    });
}

// -- build benchmarks --

fn bench_tiny_glob_set_build(c: &mut Criterion) {
    c.bench_function("tiny_glob_set_build", |b| {
        b.iter(|| {
            let mut builder = glob_set::TinyGlobSetBuilder::new();
            for pat in PATTERNS {
                builder.add(glob_set::Glob::new(pat).unwrap());
            }
            builder.build().unwrap()
        });
    });
}

fn bench_glob_set_build(c: &mut Criterion) {
    c.bench_function("glob_set_build", |b| {
        b.iter(|| {
            let mut builder = glob_set::GlobSetBuilder::new();
            for pat in PATTERNS {
                builder.add(glob_set::Glob::new(pat).unwrap());
            }
            builder.build().unwrap()
        });
    });
}

fn bench_globset_build(c: &mut Criterion) {
    c.bench_function("globset_build", |b| {
        b.iter(|| {
            let mut builder = globset::GlobSetBuilder::new();
            for pat in PATTERNS {
                builder.add(globset::Glob::new(pat).unwrap());
            }
            builder.build().unwrap()
        });
    });
}

// -- Upstream ripgrep globset benchmarks --
// Sourced from https://github.com/BurntSushi/ripgrep/blob/master/crates/globset/benches/bench.rs

const EXT: &str = "some/a/bigger/path/to/the/crazy/needle.txt";
const EXT_PAT: &str = "*.txt";

const SHORT: &str = "some/needle.txt";
const SHORT_PAT: &str = "some/**/needle.txt";

const LONG: &str = "some/a/bigger/path/to/the/crazy/needle.txt";
const LONG_PAT: &str = "some/**/needle.txt";

const MANY_SHORT_GLOBS: &[&str] = &[
    ".*.swp",
    "tags",
    "target",
    "*.lock",
    "tmp",
    "*.csv",
    "*.fst",
    "*-got",
    "*.csv.idx",
    "words",
    "98m*",
    "dict",
    "test",
    "months",
];

const MANY_SHORT_SEARCH: &str = "98m-blah.csv.idx";

fn bench_ext_globset(c: &mut Criterion) {
    let set = globset::Glob::new(EXT_PAT).unwrap().compile_matcher();
    let cand = globset::Candidate::new(EXT);
    c.bench_function("ext_globset", |b| {
        b.iter(|| assert!(set.is_match_candidate(&cand)));
    });
}

fn bench_ext_glob_set(c: &mut Criterion) {
    let set = glob_set::Glob::new(EXT_PAT).unwrap().compile_matcher();
    // glob-set's GlobMatcher does literal glob_match (no implicit **/ prefix),
    // so we match against just the filename, same as what the extension strategy
    // would do inside GlobSet.
    c.bench_function("ext_glob_set", |b| {
        b.iter(|| assert!(set.is_match("needle.txt")));
    });
}

fn bench_short_globset(c: &mut Criterion) {
    let set = globset::Glob::new(SHORT_PAT).unwrap().compile_matcher();
    let cand = globset::Candidate::new(SHORT);
    c.bench_function("short_globset", |b| {
        b.iter(|| assert!(set.is_match_candidate(&cand)));
    });
}

fn bench_short_glob_set(c: &mut Criterion) {
    let set = glob_set::Glob::new(SHORT_PAT).unwrap().compile_matcher();
    let cand = glob_set::Candidate::new(SHORT);
    c.bench_function("short_glob_set", |b| {
        b.iter(|| assert!(set.is_match_candidate(&cand)));
    });
}

fn bench_long_globset(c: &mut Criterion) {
    let set = globset::Glob::new(LONG_PAT).unwrap().compile_matcher();
    let cand = globset::Candidate::new(LONG);
    c.bench_function("long_globset", |b| {
        b.iter(|| assert!(set.is_match_candidate(&cand)));
    });
}

fn bench_long_glob_set(c: &mut Criterion) {
    let set = glob_set::Glob::new(LONG_PAT).unwrap().compile_matcher();
    let cand = glob_set::Candidate::new(LONG);
    c.bench_function("long_glob_set", |b| {
        b.iter(|| assert!(set.is_match_candidate(&cand)));
    });
}

fn bench_many_short_globset(c: &mut Criterion) {
    let mut builder = globset::GlobSetBuilder::new();
    for pat in MANY_SHORT_GLOBS {
        builder.add(globset::Glob::new(pat).unwrap());
    }
    let set = builder.build().unwrap();
    c.bench_function("many_short_globset", |b| {
        b.iter(|| assert_eq!(2, set.matches(MANY_SHORT_SEARCH).len()));
    });
}

fn bench_many_short_glob_set(c: &mut Criterion) {
    let mut builder = glob_set::GlobSetBuilder::new();
    for pat in MANY_SHORT_GLOBS {
        builder.add(glob_set::Glob::new(pat).unwrap());
    }
    let set = builder.build().unwrap();
    c.bench_function("many_short_glob_set", |b| {
        b.iter(|| assert_eq!(2, set.matches(MANY_SHORT_SEARCH).len()));
    });
}

// -- GlobMap benchmarks --

fn bench_glob_map_get(c: &mut Criterion) {
    let mut builder = glob_set::GlobMapBuilder::new();
    for (i, pat) in PATTERNS.iter().enumerate() {
        builder.insert(glob_set::Glob::new(pat).unwrap(), i);
    }
    let map = builder.build().unwrap();

    c.bench_function("glob_map_get", |b| {
        b.iter(|| {
            for path in PATHS {
                map.get(*path);
            }
        });
    });
}

criterion_group!(
    benches,
    bench_tiny_glob_set,
    bench_glob_set,
    bench_globset,
    bench_tiny_glob_set_build,
    bench_glob_set_build,
    bench_globset_build,
    bench_ext_globset,
    bench_ext_glob_set,
    bench_short_globset,
    bench_short_glob_set,
    bench_long_globset,
    bench_long_glob_set,
    bench_many_short_globset,
    bench_many_short_glob_set,
    bench_glob_map_get,
);
criterion_main!(benches);
