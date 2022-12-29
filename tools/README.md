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

The [`docker-compose.yml`](./docker-compose.yml) provides a pre-configured local TCE network setup.

You can run it with the following command,

```
docker compose up -d
```

Several services are exposed:

- `jaeger`: all-in-one observability stack for distributed tracing
- `cert-spammer`: tool to submit mocked certificates to the TCE network (for benchmark)
- `tce-boot-node`: a TCE boot node
- `tce-regular-node`: multiple replicas of TCE node

> **Note**
>
> Each service has its own set of environment variables that are configurable in the `docker-compose.yml`.<br/>
> The possible configuration through environment variables are outlined by `topos tce run --help`.

The default number of TCE (non-boot) nodes is defined by the variable `replicas` in the docker-compose file, that you can later update by running the following command:

```
docker compose up -d --no-recreate --scale node=<number>
```

The `cert-spammer` can be customized through environment variables defined in the compose file in order to change the number of certificates or the frequency of their publication.
