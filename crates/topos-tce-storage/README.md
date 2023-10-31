# topos-tce-storage

The library provides the storage layer for the Topos TCE.
It is responsible for storing and retrieving the certificates, managing the
pending certificates pool and the certificate status, storing the different
metadata related to the protocol and the internal state of the TCE.

The storage layer is implemented using RocksDB.
The library is exposing multiple store that are used by the TCE.


### Architecture

The storage layer is composed of multiple stores that are used by the TCE.
Each store is describe in detail in its own module.

As an overview, the storage layer is composed of the following stores:

<picture>
 <source media="(prefers-color-scheme: dark)" srcset="https://github.com/topos-protocol/topos/assets/1394604/5bb3c9b1-ac5a-4f59-bd14-29a02163272e">
 <img alt="Text changing depending on mode. Light: 'So light!' Dark: 'So dark!'" src="https://github.com/topos-protocol/topos/assets/1394604/e4bd859e-2a6d-40dc-8e84-2a708aa8a2d8">
</picture>

#### Definitions and Responsibilities

As illustrated above, multiple `stores` are exposed in the library using various `tables`.

The difference between a `store` and a `table` is that the `table` is responsible for storing
the data while the `store` is responsible for managing the data and its access and behaviour.

Here's the list of the different stores and their responsibilities:

- The [`EpochValidatorsStore`](struct@epoch::EpochValidatorsStore) is responsible for managing the list of validators for each `epoch`.
- The [`FullNodeStore`](struct@fullnode::FullNodeStore) is responsible for managing all the persistent data such as [`Certificate`] delivered and associated `streams`.
- The [`IndexStore`](struct@index::IndexStore) is responsible for managing indexes in order to collect information about the broadcast and the network.
- The [`ValidatorStore`](struct@validator::ValidatorStore) is responsible for managing the pending certificates pool and all the transient and volatile data.

For more information about a `store`, see the related doc.

Next, we've the list of the different tables and their responsibilities:

- The [`EpochValidatorsTables`](struct@epoch::EpochValidatorsTables) is responsible for storing the list of validators for each `epoch`.
- The [`ValidatorPerpetualTables`](struct@validator::ValidatorPerpetualTables) is responsible for storing the [`Certificate`] delivered and all the persitent data related to the broadcast.
- The [`ValidatorPendingTables`](struct@validator::ValidatorPendingTables) is responsible for storing the pending certificates pool and all the transient and volatile data.
- The [`IndexTables`](struct@index::IndexTables) is responsible for storing indexes about the delivery of [`Certificate`] such as `target subnet stream`.

### Special Considerations

When using the storage layer, you need to be aware of the following:
- The storage layer is using [rocksdb](https://rocksdb.org/) as a backend, which means that this storage doesn't need external service, as `rocksdb` is embeddable kv store.
- The storage layer is using [`Arc`](struct@std::sync::Arc) to share the stores between threads. It also means that a `store` is only instantiated once.
- Some functions are batching multiple writes in one transaction. But not all functions are using it.

### Design Philosophy

The choice of using [rocksdb](https://rocksdb.org/) as a backend was made because it is a well known and battle tested database.
It is also very fast and efficient when it comes to write and read data.

Multiple `stores` and `tables` exists in order to allow admin to deal with backups or
snapshots as they see fit. You can pick and choose which `tables` you want to backup without having to backup the whole database.

By splitting the data in dedicated tables we define strong separation of concern
directly in our storage.

`RocksDB` is however not the best fit when it comes to compose or filter data based on the data
itself.

For complex queries, another database like [`PostgreSQL`](https://www.postgresql.org/) or [`CockroachDB`](https://www.cockroachlabs.com/) could be used as a Storage for projections.
The source of truth would still be [rocksdb](https://rocksdb.org/) but the projections would be stored in a relational database, allowing for more complex queries.

As mention above, the different stores are using [`Arc`](struct@std::sync::Arc), allowing a single store to be instantiated once
and then shared between threads. This is very useful when it comes to the [`FullNodeStore`](struct@fullnode::FullNodeStore) as it is used in various places but need to provides single entrypoint to the data.

It also means that the store is immutable, which is a good thing when it comes to concurrency.

The burden of managing the locks is handled by the [`async_trait`](https://docs.rs/async-trait/0.1.51/async_trait/) crate when using the [`WriteStore`](trait@store::WriteStore).

The locks are responsible for preventing any other query to mutate the data currently in processing. For more information about the locks see [`locking`](module@fullnode::locking)

The rest of the mutation on the data are handled by [rocksdb](https://rocksdb.org/) itself.

