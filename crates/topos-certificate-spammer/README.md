# Topos Certificate Spammer

## How does it work?

The Topos Certificate Spammer generates test certificate chain and sends them to one or more target nodes, specified with parameter `--target-nodes`, e.g. `--target-nodes=http://[::1]:1340,http://[::1]:1341`.
Every `--batch-interval`, a batch of certificates is generated and sent following the `--certs-per-batch` argument.

Source subnet id and list of target subnets are randomly assigned to every certificate.

When argument `--nb-batches` is specified, program will send specified number of batches and the command will gracefully shut down connections and exit. When unspecified, it will continuously generate and send batches of certificates.

The time delay in milliseconds between two batches of two certificates is set with `--batch-interval`.

Certificates are signed with secp256k1 private key, and seed for generation of `nb-subnets` private keys is infuenced by `--local-key-seed`.

The dispatching of Certificate is done through the TCE service gRPC API.

## Commands

To compile from the root `topos` workspace directory:
```
cargo build --release
```

The extended list of commands:
```
topos network spam -h
```

## Example

Continuously spam local tce node `http://[::1]:1340` with batch of 1 certificate every 2 seconds. Certificate target subnet list is empty:
```
topos network spam 
```

Spam two tce target nodes with 3 batches (every batch containing 5 certificate with 2 possible source subnet id), 
 also specifying two possible target subnets for every generated certificate:
``` 
topos network spam --nb-subnets=2  --cert-per-batch=5 --nb-batches=3 --target-nodes=http://[::1]:1340,http://[::1]:1341 --target-subnets=0x3bc19e36ff1673910575b6727a974a9abd80c9a875d41ab3e2648dbfb9e4b518,0xa00d60b2b408c2a14c5d70cdd2c205db8985ef737a7e55ad20ea32cc9e7c417c 
```


Alternatively environment variables could be used instead of command line arguments to configure Topos Certificate Spammer:
```
TOPOS_NETWORK_SPAMMER_NUMBER_OF_CERTIFICATES
TOPOS_NETWORK_SPAMMER_TARGET_NODES
TOPOS_NETWORK_SPAMMER_SIGNING_KEY
TOPOS_NETWORK_SPAMMER_INTERVAL
TOPOS_NETWORK_SPAMMER_TARGET_SUBNETS
```

## Instrumentation

By specifying `--otlp-agent` and `--otlp-service-name` cli options, instrumentation event `NewTestCertificate` will be observable from Otlp/Jaeger.
