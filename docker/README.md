# FleetingDNS Local Stack

Quick start instructions for running the prototype compose setup.

## Prerequisites
- Docker with the Compose plugin (tested with Docker 20.10+)
- Free ports 53/udp, 53/tcp, 3000, 3100, 9090 and 9464
- On Linux, ensure your user is in the `docker` group or run commands with `sudo`

## Launch the stack
Run the following from the repository root:

```bash
cd docker
# build images and start in the background
docker compose up -d --build
```

Alternatively run `bash scripts/compose_start.sh` from the repository root to
pull the latest images and start the stack.

The first run downloads images and compiles the Rust binaries, so it may take a few minutes.

## Access Grafana
Visit [http://localhost:3000](http://localhost:3000) once the services are up.
The default credentials are `admin` / `admin`.

## Tail service logs
Use `docker compose logs -f` to follow all logs or target a specific service:

```bash
# follow everything
docker compose logs -f

# only dnsd logs
docker compose logs -f dnsd
```

Stop everything with `docker compose down`.

## Demo: register a slot
Once the stack is running, execute `scripts/roundtrip_demo.sh`.
This script:
1. Inserts a demo record in Redis.
2. Waits for `dnsd` to resolve `demo.fdns.run`.
3. Verifies the `edgehub` TLS listener.
4. Performs an HTTP request round-trip through the resolved IP.

