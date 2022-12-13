# Glossary

- BABE: (Blind Assignment for Blockchain Extension). The consensus algorithm validators use as block production mechanism. See [the Polkadot wiki][0] for more information.
- Extrinsic: An element of a relay-chain block which triggers a specific entry-point of a runtime module with given arguments.
- GRANDPA: (Ghost-based Recursive ANcestor Deriving Prefix Agreement). The algorithm validators uses to guarantee finality of the subnet. See [the Polkadot wiki][0] for more information.
- Pallet: A component of the Runtime logic, encapsulating storage, routines, and entry-points.
- Runtime: The subnet state machine.
- Runtime API: A means for the node-side behavior to access structured information based on the state of a fork of the blockchain.
- Worker: A long-running task which is responsible for carrying out a particular category of subprotocols, e.g., DKG is a Topos subprotocol implemented as a Worker.
- Validator: Specially-selected node in the network who is responsible for validating subnet block and issuing attestations about their validity.

Also of use is the [Substrate Glossary](https://substrate.dev/docs/en/knowledgebase/getting-started/glossary).

[0]: https://wiki.polkadot.network/docs/learn-consensus
