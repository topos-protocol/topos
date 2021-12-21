# Certificate spammer

## How does it work?
The Certificate spammer generates multiple Certificate per second defined by `--cert-per-second <number>`.

The Certificates are dispatched to multiple nodes listed in the file given as argument `--target-nodes cert-spammer/example_target_nodes.json`.

The dispatching of Certificate is simply done with http requests to the TCE nodes API endpoint.

## Commands

```
# To compile
cargo build --release cert-spammer

# The extended list of commands
cert-spammer -h
```

## Example

```
# Path to list of nodes by argument
cert-spammer --cert-per-sec 1000 --target-nodes list_nodes_example.json

# Path to list of nodes by environment variable
TARGET_NODES_PATH="path/to/list_nodes_example.json" cert-spammer --cert-per-sec 1000
```

### Format for the target list of TCE nodes

The format for the list of nodes can be `.json` or `.toml` as the following.
This is the list of the TCE nodes which will receive the spam of Certificate.


```json
{
    "nodes": [
        "127.0.0.1:8080",
        "127.0.0.1:8083",
        "127.0.0.1:8082",
        "127.0.0.1:8085",
        "127.0.0.1:8089"
    ]
}
```

```toml
nodes = [
  "127.0.22.1:8080",
  "127.0.3.1:8081",
  "127.0.2.1:8082",
  "127.0.4.1:8083",
]
```
