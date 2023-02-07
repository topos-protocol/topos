# CertificatesCollector

The `CertificatesCollector` is responsible for fetching and validating certificates by gathering the certificate data across peers, verifying it and persist it if everything is ok.

## General design

Upon receiving a `SourceStreamPosition` from the `CheckpointsCollector`, the `CertificatesCollector` will check the difference between the current position that the local node have, and the expected position reported by the `CheckpointsCollector`. The `CertificatesCollector` will then ask the peers of the network to retrieve the list of `CertificateId` to sync, in order. Upon receiving those responses,

<!-- ## Internal design -->
