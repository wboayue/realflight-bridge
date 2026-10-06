use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use realflight_bridge::bridge::remote::{Request, RequestType, Response, ResponseStatus};
use realflight_bridge::{
    ControlInputs, decode_frame, decode_simulator_state, encode_frame, encode_frame_into,
};

static SIM_STATE_RESPONSE: &str = include_str!("../testdata/responses/return-data-200.xml");

/// Length prefix size of a remote frame.
const FRAME_HEADER_LEN: usize = 4;

fn exchange_request() -> Request {
    Request {
        request_type: RequestType::ExchangeData,
        payload: Some(ControlInputs {
            channels: [0.5, 0.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        }),
    }
}

fn state_response() -> Response {
    Response {
        status: ResponseStatus::Success,
        payload: Some(decode_simulator_state(SIM_STATE_RESPONSE).unwrap()),
    }
}

fn bench_encode_request(c: &mut Criterion) {
    let request = exchange_request();
    c.bench_function("bench_encode_frame_request", |b| {
        b.iter(|| black_box(encode_frame(black_box(&request)).unwrap()))
    });

    let mut buf = Vec::new();
    c.bench_function("bench_encode_frame_into_request", |b| {
        b.iter(|| {
            encode_frame_into(black_box(&request), &mut buf).unwrap();
            black_box(&buf);
        })
    });
}

fn bench_encode_response(c: &mut Criterion) {
    let response = state_response();
    c.bench_function("bench_encode_frame_response", |b| {
        b.iter(|| black_box(encode_frame(black_box(&response)).unwrap()))
    });

    let mut buf = Vec::new();
    c.bench_function("bench_encode_frame_into_response", |b| {
        b.iter(|| {
            encode_frame_into(black_box(&response), &mut buf).unwrap();
            black_box(&buf);
        })
    });
}

fn bench_decode(c: &mut Criterion) {
    let request = encode_frame(&exchange_request()).unwrap();
    c.bench_function("bench_decode_frame_request", |b| {
        b.iter(|| {
            let decoded: Request = decode_frame(black_box(&request[FRAME_HEADER_LEN..])).unwrap();
            black_box(decoded)
        })
    });

    let response = encode_frame(&state_response()).unwrap();
    c.bench_function("bench_decode_frame_response", |b| {
        b.iter(|| {
            let decoded: Response = decode_frame(black_box(&response[FRAME_HEADER_LEN..])).unwrap();
            black_box(decoded)
        })
    });
}

criterion_group!(
    benches,
    bench_encode_request,
    bench_encode_response,
    bench_decode,
);
criterion_main!(benches);
