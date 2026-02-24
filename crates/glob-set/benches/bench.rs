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

criterion_group!(
    benches,
    bench_glob_set,
    bench_globset,
    bench_glob_set_build,
    bench_globset_build
);
criterion_main!(benches);
