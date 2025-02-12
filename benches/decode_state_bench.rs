use criterion::{black_box, criterion_group, criterion_main, Criterion};

use realflight_bridge::{decode_simulator_state, extract_element, extract_elements};

static SIM_STATE_RESPONSE: &str = include_str!("../testdata/responses/return-data-200.xml");

///
/// Benchmark function for naive substring approach
///
fn bench_decode_state(c: &mut Criterion) {
    c.bench_function("bench_decode_state", |b| {
        b.iter(|| {
            let extracted = decode_simulator_state(black_box(SIM_STATE_RESPONSE));
            black_box(extracted)
        })
    });
}

fn bench_extract_element(c: &mut Criterion) {
    c.bench_function("bench_extract_element", |b| {
        b.iter(|| {
            let extracted = extract_element(
                black_box("m-resetButtonHasBeenPressed"),
                black_box(SIM_STATE_RESPONSE),
            );
            black_box(extracted)
        })
    });
}

fn bench_extract_elements(c: &mut Criterion) {
    c.bench_function("bench_extract_elements", |b| {
        b.iter(|| {
            let extracted = extract_elements(black_box(SIM_STATE_RESPONSE));
            black_box(extracted)
        })
    });
}

criterion_group!(
    benches,
    bench_decode_state,
    bench_extract_element,
    bench_extract_elements
);
criterion_main!(benches);
