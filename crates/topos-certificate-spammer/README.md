# Topos Certificate Spammer

## How does it work?

The Topos Certificate Spammer generates multiple Certificate per batch defined by `--cert-per-batch <number>`.
The time delay in seconds between batches is controlled by `--batch-time-interval`.

The Certificates are dispatched to one node (using `--target-node <tce endpoint http address>`) or multiple nodes listed in the file given as
argument `--target-node-list cert-spammer/example_target_nodes.json`.

The dispatching of Certificate is done through the TCE service gRPC API.

## Commands

```
# To compile from the root workspace directory
cargo build --release

# The extended list of commands
topos network spam -h
```

## Example

```
# Spam target node with certs
topos network spam --cert-per-batch 1000 --target-node http://[::1]:1340
```

```
# Spam list of target nodes with certs
topos network spam --cert-per-batch 1000 --target-nodes-path example_target_nodes.json
```
Alternatively environment variables could be used instead of command line arguments to configure Topos Certificate Spammer: `TARGET_NODE`, `TARGET_NODES_PATH`, `CERT_PER_BATCH`.

### Format for the target list of TCE nodes

The format for the list of nodes can be `.json` or `.toml` as the following.
This is the list of the TCE nodes which will receive the spam of Certificate.

```json
{
  "nodes": [
    "http://[::1]:1340",
    "http://[::1]:1341",
    "http://[::1]:1342",
    "http://[::1]:1343",
    "http://[::1]:1344"
  ]
}
```

```toml
nodes = [
    "http://[::1]:1340",
    "http://[::1]:1341",
    "http://[::1]:1342",
]
```
