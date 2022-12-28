# Docker

The `TOOLCHAIN_VERSION` needs to be defined through `(stable|nightly-2022-07-20|...)`

Targeted docker build commands follow the following pattern:

```
docker build . --build-arg GITHUB_TOKEN=*** --build-arg TOOLCHAIN_VERSION=[...] --target [TARGET]
```

The development workflow through docker.

```
docker build . ... --target (build|test|fmt|lint)
```
# Docker Compose Setup

The [`docker-compose.yml`](./docker-compose.yml) provides a pre-configured TCE local network setup.

You can run it with the following command,

```
docker compose up -d
```

Several services are exposed,

- Instrumentation with `jaeger` on `ip:port`
- Spammer of Certificate `cert-spammer` which is used to benchmark the TCE
- `tce-boot-node` which is a TCE bootnode
- Several replicas `tce-regular-node` which are the TCE participants

Each service are having their own set of environment variable that are configurable in the `docker-compose.yml`.

The default number of TCE nodes is defined by the variable `replicats` in the docker-compose file, that you can specify in the command line

```
docker compose up -d --no-recreate --scale node=<number>
```

The `cert-spammer` can be customized through environment variable defined in the compose file in order to change the number of certificates or the frequency of publish.
