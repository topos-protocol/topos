#  TCE Node


Main file is located at `src/tce_node_app.rs`, Cargo - root of the repo.

## Components

### net

Provides a simplified, though still low level, interface for interacting with the underlying peer-to-peer network layer.

It is based on libp2p, implementation built around concepts of protocols and behaviours.

Handles advertisement of provided services, using DHT.

Performs auto-discovery of the network using Kademlia, Identify and DHT.

### api

Umbrella module for all non-TCENetwork APIs of the node. 

Web API is based on hyper. 
