# Docker Compose Setup

Run a minimal local setup from the current `./tools/` folder.

```
docker compose up -d
```

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
