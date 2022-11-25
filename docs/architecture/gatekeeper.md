# Gatekeeper

The `Gatekeeper` is a central piece of the node. It has the role of defining which peers can participating and which nodes needs to be listen to.

## General Design

The `Gatekeeper` will have a local inmemory state representing the list of TCE nodes allowed to participate but also a list of every subnets allowed to submit certificates.

The `Gatekeeper` will manage the two lists using multiple mechanisms that are not defined for now.

The `Gatekeeper` can receive and respond to commands in order to provide information to other components:

- Update the configuration of the `Gatekeeper`
- Request a full list of every peers
- Request a random list of peers
- Request a full list of every subnets

## Internal design

The `Gatekeeper` isn't fully designed for now but a first iteration will be to expose a simple gRPC API to push update of the list of TCE participants.

In the future we'll need to find solution to gather those information at the source of thruth. The goal here is to expose methods that can be used by any components and in a near future, replace the internal implementation.

This component will be responsible for exposing the lists of peers/subnets but it'll be also responsible for maintaining some kind of reputation for peers that we're connecting with.

### Peer list

The `Peer` list maintained by the `Gatekeeper` will be a simple list of `PeerId`.

### Subnet list

The `Subnet` list maintained by the `Gatekeeper` will be a simple list of `SubnetId`.
