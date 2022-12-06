# Synchronizer

The `Synchronizer` is responsible for organizing the components involved in the sync process.

## General design

The main goal of the `Synchronizer` is to be the entry point of the `Runtime` to start and drive the `sync` process of the node. The `Synchronizer` is responsible for spawning and driving the components involved in the `sync` process by listening events coming from those components but also sending command to them if needed.

The `Synchronizer` will have some commands used by the `Runtime`, and expose events to notify the `Runtime` about important actions, events or issues during the `sync` process.

The `Synchronizer` manages two main subcomponents which are `CheckpointsCollector` and `CertificatesCollector`.

## Internal design
