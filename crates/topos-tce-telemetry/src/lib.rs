//use libp2p::PeerId;
use opentelemetry::trace::TraceFlags;
use opentelemetry::KeyValue;
use opentelemetry::{
    global,
    trace::{SpanBuilder, SpanKind, TraceId, Tracer},
};
use topos_core::uci::CertificateId;

const JAEGER_HEADER: &str = "topos-trace-id";
#[allow(unused)]
const JAEGER_BAGGAGE_PREFIX: &str = "toposctx-";
#[allow(unused)]
const TRACE_FLAG_DEBUG: TraceFlags = TraceFlags::new(0x04);

lazy_static::lazy_static! {
    static ref JAEGER_HEADER_FIELD: [String; 1] = [JAEGER_HEADER.to_string()];
}

pub fn init_tracer(agent_endpoint: &String, service_name: &str) {
    log::info!("Initialize jaeger tracer agent for {:?}", agent_endpoint);
    global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
    match opentelemetry_jaeger::new_pipeline()
        .with_agent_endpoint(agent_endpoint)
        .with_service_name(service_name)
        .build_batch(opentelemetry::runtime::Tokio)
    {
        Ok(provider) => {
            // Initialize the singleton
            global::set_tracer_provider(provider);
        }
        Err(e) => {
            log::error!("Fail to initialize tracer {}", e);
        }
    }
}

pub fn span_cert_delivery(
    peer_id: String,
    cert: &CertificateId,
    start: std::time::SystemTime,
    end: std::time::SystemTime,
    attr: Vec<KeyValue>,
) {
    let mut trace_id: [u8; 16] = [0; 16];
    trace_id[..cert.len()].copy_from_slice(cert.as_bytes());
    let tracer = global::tracer("cert-latency");
    let _span = tracer.build(
        SpanBuilder {
            //name: p.to_base58().into(),
            name: peer_id.into(),
            span_kind: Some(SpanKind::Server),
            ..Default::default()
        }
        .with_trace_id(TraceId::from_bytes(trace_id))
        .with_start_time(start)
        .with_end_time(end)
        .with_attributes(attr),
    );
}
