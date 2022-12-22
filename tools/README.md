### Docker

The development workflow can be done with docker.

The `TOOLCHAIN_VERSION` needs to be defined through `(stable|nightly-2022-07-20|...)`

Targeted docker build commands follow the following pattern:

```
docker build . --build-arg GITHUB_TOKEN=*** --build-arg TOOLCHAIN_VERSION=[...] --target [TARGET]
```

```
docker build . ... --target (build|test|fmt|lint)
```
