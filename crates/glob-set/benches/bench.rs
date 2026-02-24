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
    bench_glob_map_get,
);
criterion_main!(benches);
