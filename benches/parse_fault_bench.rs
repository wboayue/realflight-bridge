use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quick_xml::events::Event;
use quick_xml::Reader;

static XML_DATA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"
                   xmlns:SOAP-ENC="http://schemas.xmlsoap.org/soap/encoding/"
                   xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
                   xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <SOAP-ENV:Body>
        <SOAP-ENV:Fault>
            <faultcode>SOAP-ENV:Server</faultcode>
            <faultstring>Error setting channel values</faultstring>
            <detail>RealFlight Link controller has not been instantiated</detail>
        </SOAP-ENV:Fault>
    </SOAP-ENV:Body>
</SOAP-ENV:Envelope>"#;

///
/// 1) Naive Substring Extraction
///
fn naive_extract_detail(xml: &str) -> Option<String> {
    let start_tag = "<detail>";
    let end_tag = "</detail>";

    let start_pos = xml.find(start_tag)?;
    let end_pos = xml.find(end_tag)?;

    // The detail content starts right after `<detail>` ends
    let detail_start = start_pos + start_tag.len();
    if detail_start >= end_pos {
        return None;
    }

    Some(xml[detail_start..end_pos].to_string())
}

///
/// 2) quick-xml Streaming Extraction
///
fn quick_xml_extract_detail(xml_data: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml_data);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf).ok()? {
            Event::Start(e) => {
                if e.name().as_ref() == b"detail" {
                    // The next event should be the text in <detail>
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                        return t.unescape().ok().map(|t| t.to_string());
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

///
/// Benchmark function for naive substring approach
///
fn bench_naive(c: &mut Criterion) {
    c.bench_function("naive_substring_extract", |b| {
        b.iter(|| {
            let extracted = naive_extract_detail(black_box(XML_DATA));
            black_box(extracted)
        })
    });
}

///
/// Benchmark function for quick-xml approach
///
fn bench_quick_xml(c: &mut Criterion) {
    c.bench_function("quick_xml_extract", |b| {
        b.iter(|| {
            let extracted = quick_xml_extract_detail(black_box(XML_DATA));
            black_box(extracted)
        })
    });
}

criterion_group!(benches, bench_naive, bench_quick_xml);
criterion_main!(benches);
