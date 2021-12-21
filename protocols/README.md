# Topos / [TCE](../../README.md) / Protocols

Topos Protocols are the building blocks for interoperability between subsystems and across peers.

## Tracing

TCE Nodes have integrated Jaeger / Open Trace spans built in for monitoring key metrics.

For local execution a stand alone Jaeger server can be run easily using the following command, though a more robust and scalable solution is recommended for production loads:

```docker
docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest
```

Once running this Jaeger server can be located in the brower at:

```text
http://localhost:16686/
```

Open telemetry enables the creation of nested spans to represent the call stack of the application and can include both error and event metadata within the submitted trace entries.

The following provides some simple examples of including metadata:

```rust
let tracer = config.tracer.to_owned();
tracer.in_span("step-1", |cx| {
    tracer.in_span("step-2-1", |cx| {
        cx.span().add_event("something happened", vec![KeyValue::new("key1", "value1")]);
    });
    tracer.in_span("step-2-2", |cx| {
        let span = cx.span();
        span.set_status(StatusCode::Error, "some err".to_owned())
    })
});
```

## Testing

Integration tests. Snippets to run from project root.
Current integration tests cover solely the totality property of the TCE.

```shell
cargo test -p topos-tce-protocols-reliable-broadcast --test totality
```
