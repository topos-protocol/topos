# CheckpointsCollector

The `CheckpointsCollector` is the component that will communicate with other peers to negotiate a network checkpoint to sync with.

## General design

The `CheckpointsCollector` will use the P2P network to ask for the `Checkpoints` of others peers. The selected peers will be provided by the `Gatekeeper`, returning a chunk of random peers to communicate with.
This component asks for Peer's `Checkpoints`. `Peers` will send their current checkpoint as a response. This is the responsibility of the `CheckpointsCollector` to communicate with others peers to build and find a network `Checkpoint` that can be use to `sync`.

## Internal design

The `CheckpointsCollector`, as describe above, has a main goal of negotiating a common checkpoint to start syncing the node.

The first thing to do for the `CheckpointsCollector` is to contact the `Gatekeeper` for a list of peers to sync with.
Upon receiving the list of peers, the `CheckpointsCollector` will open connections and send a message to ask for checkpoint, it is pretty streightforward but it's the first step to define if the peers are trustable to sync.

The message sent to every peers look like this:

```protobuf
message CheckpointRequest {
  bool content = 1;
}

message CheckpointResponse {
  repeatable SourceStreamPosition heads = 1;
  CheckpointContents content = 2;
}

message CheckpointContents {
  map<CertificateId, Certificate> content = 1;
  int32 count = 2;
}
```

```
SEND to   peer1: CheckpointRequest { content: false }
RECV from peer1: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], content: CheckpointContents { content: { .. }, count: 10 } }

```

Upon receiving the response of every peers, the `CheckpointsCollector` will gather every `CheckpointResponse` and analyze them in order to decide if it can trust the peers that respond.
For every subnets contained in the `CheckpointResponse` of every peers, the `CheckpointsCollector` will ask the local storage for the current node `SourceStreamPosition` head.
For every `SourceStreamPosition` that are lower than our current head, we don't have to sync.

### TTL on CheckpointRequest

If a peer doesn't respond following a TTL on the request, this peer will be tagged as byzantine.
If the `CheckpointsCollector` find itself having less responses than expected regarding a preconfigured threshold, it can decide to dump every response and ask for a new set of peers from the `Gatekeeper`. (Informing the `Gatekeeper` with the list of peers that didn't respond in time).

### Selecting the smallest Position across responses per subnet

Because of the distributed and asynchronous delivery of certificates during the broadcast, some peers of our set of sync peers can be late or in advance compare to others. In order to sync and have a consistent view of the network, the node needs to detect this pattern and chose the smallest position in the responses in order to ask for the `SourceStreamPosition` for that subnet at that position.

```
Responses:
  RECV from peer1: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], .. }
  RECV from peer2: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], .. }
  RECV from peer3: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xef", position: 9}, ..], .. }

Problem:  Received different position for the same subnet
Fallback: Requesting `SourceStreamPosition` for the position 9 to check for consistency
```

```
Responses:
  RECV from peer1: SourceStreamPositionResponse { subnet_id: "0x0a", certificate_id: "0xef", position: 9}
  RECV from peer2: SourceStreamPositionResponse { subnet_id: "0x0a", certificate_id: "0xef", position: 9}
  RECV from peer3: SourceStreamPositionResponse { subnet_id: "0x0a", certificate_id: "0xef", position: 9}

Result: This set of peers are sync and we have a consistent point in the stream to sync. Every peers have the same certificate_id and position.
```

### Inconsistent CheckpointResponses

Apart from TTL there are some cases which can represent an inconsistent set of `CheckpointResponse`.

#### Receiving different certificate_id for the same Position

The `Position` of a `certificate` in the `SourceStream` of a subnet is guaranteed to be the same across all TCE node. It is enforced by the topos protocol itself and more precisely by the `Broadcast` mechanisms.
When receiving inconsistent `SourceStreamPosition` for a `subnet` across multiples `CheckpointResponse`, if it hits a threshold, the node needs to dump every responses and fetch a new set of peers from the `Gatekeeper`.

```
Responses:
  RECV from peer1: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], .. }
  RECV from peer2: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], .. }
  RECV from peer3: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xef", position: 10}, ..], .. }

Problem:  Receiving different cert for the same position of the same subnet
Fallback: Requesting another batch of TCE nodes until receiving consistent response
```

#### Receiving different Position for the same certificate_id

The `Position` of a `certificate` in the `SourceStream` of a subnet is guaranteed to be the same across all TCE node. It is enforced by the topos protocol itself and more precisely by the `Broadcast` mechanisms.
When receiving inconsistent `SourceStreamPosition` for a `subnet` across multiples `CheckpointResponse`, if it hits a threshold, the node needs to dump every responses and fetch a new set of peers from the `Gatekeeper`.

```
Responses:
  RECV from peer1: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], .. }
  RECV from peer2: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 10}, ..], .. }
  RECV from peer3: CheckpointResponse { heads: [SourceStreamPosition { subnet_id: "0x0a", certificate_id: "0xba", position: 11}, ..], .. }

Problem:  Receiving different position for the same cert of the same subnet
Fallback: Requesting another batch of TCE nodes until receiving consistent response
```
