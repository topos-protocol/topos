# AddrAnnouncer

This `AddrAnnouncer` is responsible for publishing on a time based the node addresses into the DHT.
Its job is simple and pretty straightforward, publishing the addresses on the DHT to allow any node to request with the `PeerId` the addresses.

## General design

This `AddrAnnouncer` doesn't expose complex logic nor full client to interact with it. This `AddrAnnouncer` is managed by the `topos-p2p` runtime and can't be accessible to other `AddrAnnouncer`.
Some actions may be performed on this `AddrAnnouncer` to drive its behavior such as:

- Start/Pause/Stop
- Changing the TTL of the publish method
- Adding/removing addresses from the addrs pool

## Internal design

The `AddrAnnouncer` is using futures to publish addresses into the DHT, those futures will have a TTL to detect publication failure.
The `AddrAnnouncer` will also need to deal with `PutRecord` response to notify the `Runtime` in case of failure. Every time a publication is successful, the `AddrAnnouncer` will start a new future to republish the addresses based on the TTL defined in its state.
The `AddrAnnouncer` will listen for command coming from the `Runtime`, those commands will drive the behavior of the `AddrAnnouncer`.

- `Start` command will start the `AddrAnnouncer` and the announcement process, receiving a new `Start` command should do nothing if already started.
- `Stop` command will ask the `AddrAnnouncer` to shutdown, if an announcement is being processed the `AddrAnnouncer` will let it finish before stopping
- `Pause` command will stop the announcement but keep the `AddrAnnouncer` alive
- `TTLChanged` command will update the TTL for the next announcement
- `AddressesUpdated` command will update the addresses to be published for the next announcement

### Publications

To publish the `AddrsRecord` for the node the `AddrAnnouncer` needs to sign the addresses pool with the node private key to ensure that the producer of the value can be verified by any peer.
A peer requesting the `AddrsRecord` of another peers will receive the response containing both the `addresses` and the `signature`. The requesting peer will be able to verify the payload by using the `PeerId` to verify the `signature`.
Thus, if verification succeed, the addresses pool of the `Peer` can be trusted and added to the peer's known addresses.

The publication of the `AddrsRecord` will be handled by the Kademlia DHT bounded to the `Discovery` behavior. The publication (and re-publication) interval will be handled by the Kademlia global config.
We can define an expiration when publishing the `AddrsRecord` so that our local record expire and is not re-publish anymore, or we can use the DHT TTL expiration.

### Replications

The replication of the `AddrsRecord` will follow the default configuration of the Kademlia DHT. When the `AddrAnnouncer` needs to publish a new `AddrsRecord` the Quorum needs to be `All` to guarantee a good replication on the DHT nodes.
The re-replication, as re-publication, is handled by the Kademlia DHT.

### Expiration

If we don't specify any expiration on the `AddrsRecord` the record stored locally will never expire but it is still replicated with the (global) configured record TTL.
If the node publishing becomes offline, the record will expire after the defined TTL
