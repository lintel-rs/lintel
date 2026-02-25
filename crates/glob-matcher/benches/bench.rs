#![allow(clippy::unwrap_used)]

use criterion::{Criterion, criterion_group, criterion_main};

const PATH: &str = "some/a/bigger/path/to/the/crazy/needle.txt";
const GLOB: &str = "some/**/needle.txt";

#[inline]
fn glob(pat: &str, s: &str) -> bool {
    let pat = glob::Pattern::new(pat).expect("valid glob pattern");
    pat.matches(s)
}

#[inline]
fn globset(pat: &str, s: &str) -> bool {
    let pat = globset::Glob::new(pat)
        .expect("valid glob")
        .compile_matcher();
    pat.is_match(s)
}

fn glob_matcher_crate(b: &mut Criterion) {
    b.bench_function("glob_matcher_crate", |b| {
        b.iter(|| assert!(glob_matcher::glob_match(GLOB, PATH)));
    });
}

fn glob_match_crate(b: &mut Criterion) {
    b.bench_function("glob_match_crate", |b| {
        b.iter(|| assert!(glob_match::glob_match(GLOB, PATH)));
    });
}

fn glob_crate(b: &mut Criterion) {
    b.bench_function("glob_crate", |b| b.iter(|| assert!(glob(GLOB, PATH))));
}

fn globset_crate(b: &mut Criterion) {
    b.bench_function("globset_crate", |b| b.iter(|| assert!(globset(GLOB, PATH))));
}

criterion_group!(
    benches,
    globset_crate,
    glob_crate,
    glob_matcher_crate,
    glob_match_crate,
);
criterion_main!(benches);
