# topos-tce-broadcast

Implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)

This crate is designed to be used as a library in the TCE implementation.
It covers the Reliable Broadcast part of the TCE, which is the core of the TCE.
It doesn't handle how messages are sent or received, nor how the certificates are stored.
It is designed to be used with any transport and storage implementation, relying on the
`ProtocolEvents` and `DoubleEchoCommand` to communicate with the transport and storage.

The reliable broadcast allows a set of validators to agree on a set of messages in order to
reach agreement about the delivery of a certificate.

Each certificates need to be broadcast to the network, and each validator needs to
receive a threshold of messages from the other validators.
The thresholds are defined by the `ReliableBroadcastParams` and correspond to the minimum number of
validators who need to agree on one certificate in order to consider it delivered.

This crate is responsible for validating and driving the broadcast of every certificates.

### Input

The input of the broadcast is a certificate to be broadcast. It can be received from
the transport layer, or from the storage layer (from the pending tables).

The transport layer can be anything from p2p network to API calls.

Other inputs are the messages received from the transport layer, coming from other validators.
They're `Echo` and `Ready` signed messages.

### Output

The outcome of the broadcast is either a certificate delivered or a failure on the delivery.

The implementation is based on the paper: [Topos: A Secure, Trustless, and Decentralized Interoperability Protocol](https://arxiv.org/pdf/2206.03481.pdf)

