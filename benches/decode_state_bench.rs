use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use realflight_bridge::{
    ControlInputs, decode_simulator_state, encode_control_inputs, extract_element,
};

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

fn bench_encode_control_inputs(c: &mut Criterion) {
    let inputs = ControlInputs {
        channels: [0.5, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    };
    c.bench_function("bench_encode_control_inputs", |b| {
        b.iter(|| {
            let encoded = encode_control_inputs(black_box(&inputs));
            black_box(encoded)
        })
    });
}

criterion_group!(
    benches,
    bench_decode_state,
    bench_extract_element,
    bench_encode_control_inputs,
);
criterion_main!(benches);
