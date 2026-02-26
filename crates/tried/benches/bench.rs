use criterion::{Criterion, criterion_group, criterion_main};
use tried::DoubleArray;
use tried::builder::DoubleArrayBuilder;

#[allow(clippy::cast_possible_truncation)]
fn generate_keyset(n: usize) -> Vec<(String, u32)> {
    let mut keys: Vec<(String, u32)> = (0..n).map(|i| (format!("key_{i:06}"), i as u32)).collect();
    keys.sort();
    keys
}

fn bench_build(c: &mut Criterion) {
    let keyset = generate_keyset(10_000);

    let mut group = c.benchmark_group("build");
    group.bench_function("10k_keys", |b| {
        b.iter(|| DoubleArrayBuilder::build(keyset.as_slice()));
    });
    group.finish();
}

fn bench_exact_match(c: &mut Criterion) {
    let keyset = generate_keyset(10_000);
    let da_bytes = DoubleArrayBuilder::build(keyset.as_slice()).expect("build failed");
    let da = DoubleArray::new(da_bytes);

    let mut group = c.benchmark_group("exact_match");
    group.bench_function("10k_keys", |b| {
        b.iter(|| {
            for (key, _) in &keyset {
                assert!(da.exact_match_search(key).is_some());
            }
        });
    });
    group.finish();
}

fn bench_common_prefix(c: &mut Criterion) {
    let keyset = generate_keyset(10_000);
    let da_bytes = DoubleArrayBuilder::build(keyset.as_slice()).expect("build failed");
    let da = DoubleArray::new(da_bytes);

    let mut group = c.benchmark_group("common_prefix");
    group.bench_function("10k_keys", |b| {
        b.iter(|| {
            for (key, _) in &keyset {
                let _ = da.common_prefix_search(key).count();
            }
        });
    });
    group.finish();
}

criterion_group!(benches, bench_build, bench_exact_match, bench_common_prefix);
criterion_main!(benches);
