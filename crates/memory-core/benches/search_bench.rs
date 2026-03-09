use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use memory_core::{SaveParams, SearchParams, Store};

const CORPUS_SIZES: &[usize] = &[100, 1_000, 10_000];

fn build_corpus(store: &Store, n: usize) {
    for i in 0..n {
        let (key, value) = match i % 5 {
            0 => (
                format!("architecture/pattern-{i}"),
                format!("The system uses microservices with async message passing for component {i}. Configuration stored in TOML."),
            ),
            1 => (
                format!("bug/fix-{i}"),
                format!("Fixed null pointer in component {i}. Root cause was uninitialized memory in the worker thread pool."),
            ),
            2 => (
                format!("api/endpoint-{i}"),
                format!("REST endpoint /api/v{i}/resource accepts POST with JSON body. Authentication via bearer token."),
            ),
            3 => (
                format!("database/migration-{i}"),
                format!("Migration {i}: added index on accessed_at column. Improves query performance by 3x for large datasets."),
            ),
            _ => (
                format!("config/setting-{i}"),
                format!("Configuration key {i}: controls retry behavior with exponential backoff up to 30 seconds."),
            ),
        };
        store
            .save(SaveParams {
                key,
                value,
                ..Default::default()
            })
            .unwrap();
    }
}

/// Single-term search matching in the key column (high-weight, should rank first).
fn bench_key_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/key_match");
    for &size in CORPUS_SIZES {
        let store = Store::open_in_memory().unwrap();
        build_corpus(&store, size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                store
                    .search(black_box(SearchParams {
                        query: "microservices".into(),
                        scope: None,
                        source_type: None,
                        limit: Some(5),
                    }))
                    .unwrap()
            })
        });
    }
    group.finish();
}

/// Multi-term search across multiple columns.
fn bench_multi_term(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/multi_term");
    for &size in CORPUS_SIZES {
        let store = Store::open_in_memory().unwrap();
        build_corpus(&store, size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                store
                    .search(black_box(SearchParams {
                        query: "microservices async configuration TOML".into(),
                        scope: None,
                        source_type: None,
                        limit: Some(5),
                    }))
                    .unwrap()
            })
        });
    }
    group.finish();
}

/// Scoped search — adds scope ancestor expansion to the query.
fn bench_scoped(c: &mut Criterion) {
    let mut group = c.benchmark_group("search/scoped");
    for &size in CORPUS_SIZES {
        let store = Store::open_in_memory().unwrap();
        build_corpus(&store, size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                store
                    .search(black_box(SearchParams {
                        query: "migration index performance".into(),
                        scope: Some("/project/db".into()),
                        source_type: None,
                        limit: Some(5),
                    }))
                    .unwrap()
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_key_match, bench_multi_term, bench_scoped);
criterion_main!(benches);
