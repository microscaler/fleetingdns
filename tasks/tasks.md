# 🐳 Local‑dev Docker Composestack (prototype v0.1)

Directory layout:

```
./docker
├── docker-compose.yml
├── Dockerfile              # multi‑stage builder for all Rust bins
├── grafana/
│   ├── grafana.ini
│   ├── README.md
│   ├── provisioning/
│   │   ├── datasources/datasources.yml
│   │   └── dashboards/dashboards.yml
│   └── dashboards/
│       ├── fleetingdns-jaeger.json
│       ├── fleetingdns-loki.json
│       └── fleetingdns-unified.json
├── loki-config.yaml
├── promtail-config.yaml
├── prometheus.yml
├── otel-collector-config.yaml      # for local run
└── otel-collector-config.ci.yaml   # lightweight for GH Actions
```

---

## docker/docker-compose.yml

```yaml
version: "3.9"

x-rust-build: &rust-build
  build:
    context: ..        # repo root
    dockerfile: docker/Dockerfile
    target: runtime

services:
  dnsd:
    <<: *rust-build
    image: fleetingdns/dnsd:dev
    command: ["/app/dnsd-bin", "--addr", "0.0.0.0:53"]
    ports: ["5353:53/udp"]
    depends_on: [otel-collector]

  edgehub:
    <<: *rust-build
    image: fleetingdns/edgehub:dev
    command: ["/app/edgehub-bin"]
    depends_on: [dnsd, otel-collector]

  intake_collector:
    <<: *rust-build
    image: fleetingdns/intake:dev
    command: ["/app/intake_collector-bin"]
    depends_on: [otel-collector]

  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_USER: fdns
      POSTGRES_PASSWORD: fdns
      POSTGRES_DB: fdns
    volumes: ["pgdata:/var/lib/postgresql/data"]

  redis:
    image: redis:7-alpine
    command: ["redis-server", "--save", "", "--appendonly", "no"]

  mimir:
    image: grafana/mimir:2.10.2
    command: ["-config.file=/etc/mimir.yaml"]
    volumes: ["./mimir-local.yaml:/etc/mimir.yaml:ro"]

  otel-collector:
    image: otel/opentelemetry-collector-contrib:0.93.0
    volumes:
      - ./otel-collector-config.yaml:/etc/otel/config.yaml:ro
    command: ["--config=/etc/otel/config.yaml"]
    ports:
      - "4317:4317"   # OTLP gRPC
      - "4318:4318"   # OTLP HTTP

  prometheus:
    image: prom/prometheus:v2.52.0
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    command:
      - "--config.file=/etc/prometheus/prometheus.yml"
      - "--enable-feature=memory-snapshot-on-shutdown"
    ports: ["9090:9090"]

  loki:
    image: grafana/loki:2.9.3
    command: ["-config.file=/etc/loki-config.yaml"]
    volumes:
      - ./loki-config.yaml:/etc/loki-config.yaml:ro
    ports: ["3100:3100"]

  promtail:
    image: grafana/promtail:2.9.3
    command: ["-config.file=/etc/promtail-config.yaml"]
    volumes:
      - ./promtail-config.yaml:/etc/promtail-config.yaml:ro
      - /var/log:/var/log:ro

  grafana:
    image: grafana/grafana:10.3.1
    volumes:
      - ./grafana/grafana.ini:/etc/grafana/grafana.ini:ro
      - ./grafana/provisioning:/etc/grafana/provisioning:ro
      - ./grafana/dashboards:/var/lib/grafana/dashboards:ro
    environment:
      - GF_SERVER_HTTP_PORT=3000
    ports: ["3000:3000"]
    depends_on: [prometheus, loki, mimir]

volumes:
  pgdata:
```

---

## docker/Dockerfile (multi‑stage)

```Dockerfile
FROM rust:1.77-slim AS builder
WORKDIR /src
COPY . .
RUN cargo build --workspace --release

FROM debian:bookworm-slim AS runtime
RUN useradd -m app
WORKDIR /app
COPY --from=builder /src/target/release/*-bin /app/
USER app
CMD ["/bin/sh"]
```

---

## Minimal configs (place‑holders)

### docker/otel-collector-config.yaml

```yaml
receivers:
  otlp:
    protocols:
      grpc:
      http:
exporters:
  prometheus:
    endpoint: ":9464"
  loki:
    endpoint: "http://loki:3100/loki/api/v1/push"
service:
  pipelines:
    metrics:
      receivers: [otlp]
      exporters: [prometheus]
    logs:
      receivers: [otlp]
      exporters: [loki]
```

### docker/prometheus.yml

```yaml
global:
  scrape_interval: 10s
scrape_configs:
  - job_name: otel
    static_configs:
      - targets: ['otel-collector:9464']
```

### docker/loki-config.yaml

```yaml
auth_enabled: false
server:
  http_listen_port: 3100
```

### docker/promtail-config.yaml

```yaml
server:
  http_listen_port: 9080
positions:
  filename: /tmp/positions.yml
clients:
  - url: http://loki:3100/loki/api/v1/push
```

### docker/grafana/provisioning/datasources/datasources.yml

```yaml
apiVersion: 1
datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
  - name: Loki
    type: loki
    access: proxy
    url: http://loki:3100
  - name: Mimir
    type: prometheus
    url: http://mimir:9009/prometheus
```

*(dashboards JSON left as TODO placeholders; drop exported Grafana JSON into `grafana/dashboards/`)*

---

> **CI variant** – GitHub Actions can mount `.docker/otel-collector-config.ci.yaml` (smaller pipeline) by overriding compose file: `docker compose -f docker/docker-compose.yml -f docker/docker-compose.ci.yml up -d --build`. Create the `.ci.yml` overlay later.

🎉  Commit this folder, then run:

```bash
cd docker
docker compose up -d --build
```

`dig @localhost -p5353 test.fdns.run +short` should reply **127.0.0.1** and Grafana [http://localhost:3000](http://localhost:3000) shows Prometheus+Loki datasources pre‑wired.


---

### Codex Task List — “Local Docker-Compose Stack” (Prototype 0.3)

| ID       | Title                                                                                                  | Path / files                                                                                         | Detailed description & acceptance criteria                                                                                                                                                                                                                                                                                                                        |
|----------|--------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **T-14** | 📂 Create `docker/` scaffold + baseline `docker-compose.yml`                                           | `docker/docker-compose.yml`                                                                          | *Desc*  Add directory tree (see spec). Compose file must declare services: `dnsd`, `edgehub`, `intake_collector`, `ml_scorer`, `feed_grpc`, `feed_webhook`, `api`, `redis`, `postgres`, `mimir`, `otel-collector`, `prometheus`, `loki`, `promtail`, `grafana`. Network mode default. \n*AC*  `docker compose pull` then `docker compose config --quiet` exits 0. |
| **T-15** | 🐳  Multi-stage builder Dockerfile                                                                     | `docker/Dockerfile`                                                                                  | *Desc*  Two stages: `builder` (Rust 1.77) builds **all** `*-bin`, `runtime` (debian slim) copies into `/app/`. Add non-root user. \n*AC*  `docker build -f docker/Dockerfile .` succeeds; `docker run` prints `dnsd listening` when selecting that image. Each service must have its own Dockerfile and start as its own service.                                 |
| **T-16** | Prometheus & Otel configs                                                                              | `docker/prometheus.yml`, `docker/otel-collector-config.yaml`, `docker/otel-collector-config.ci.yaml` | *Desc*  Local config scrapes OTLP exporter; CI variant minimal (no Loki, no Mimír). \n*AC*  `docker compose up prometheus otel-collector` then `curl localhost:9090/targets` shows `otel` up.                                                                                                                                                                     |
| **T-17** | Loki + Promtail configs                                                                                | `docker/loki-config.yaml`, `docker/promtail-config.yaml`                                             | *Desc*  Loki auth\_disabled, Promtail tail `/var/log/*`. \n*AC*  Grafana datasource test returns OK for Loki when stack is up.                                                                                                                                                                                                                                    |
| **T-18** | Grafana provisioning & dashboards                                                                      | `docker/grafana/...`                                                                                 | *Desc*  Provision datasources (Prom, Loki, Mimír) and import three stub dashboards (JSON). `grafana.ini` sets admin/admin. \n*AC*  Hitting `http://localhost:3000/api/search` (basic auth) returns 3 dashboards.                                                                                                                                                  |
| **T-19** | GitHub-Actions job: compose smoke test                                                                 | `.github/workflows/compose-ci.yml`                                                                   | *Desc*  Matrix linux latest; steps: checkout → `docker compose -f docker/docker-compose.yml -f docker/docker-compose.ci.yml up -d --build` → `dig @127.0.0.1 -p5353 test.fdns.run +short` equals `127.0.0.1` → Grafana API health check. \n*AC*  Workflow passes in PR.                                                                                           |
| **T-20** | Docs: local-dev README section                                                                         | `docker/README.md`                                                                                   | *Desc*  Quick-start: prerequisites, `docker compose up -d --build`, Grafana creds, how to tail logs. \n*AC*  New developer reproduces stack in <10 min (peer review).                                                                                                                                                                                             |
| **T-21** | Wire dnsd ↔ EdgeHub end-to-end demo (docs + Docker compose setup and script to test in ci job or demo. |                                                                                                      | Demo for peer review).                                                                                                                                                                                                                                                                                                                                            |
6. **T-21** – 

> **Implementation order:** T-14 → T-15 → T-16/17 → T-18 → T-19 → T-20 → T-21.

Complete these to have a fully containerised local + CI environment wired to Prometheus/Loki/Grafana and ready for next feature iterations.


---

## When Prototype 0.2 is green

* Move EdgeHub to **TLS-wrapped OpenSSH** or `thrussh` for keyless auth.
* Start E1 series work (DNSSEC, HMAC labels).
* Parallel track: intake → Pub/Sub to set up scoring pipeline (E12/E13).

Ping me whenever you'd like deep-dive guidance on any sub-task—or if you want fresh Codex tickets for the next feature slice. Awesome progress!

---

For more details take a look at ./tasks/Rust_Codebase_Roadmap_for_FleetingDNS-FDNS_Shield.md

As well as the detailed epics in the ./docs/engineering/Epic_highlevel/E1*-*.md


