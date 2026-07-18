# llmwiki — Agent schema for fleetingdns

This wiki is the **persistent, compounding memory** for FleetingDNS — the
ephemeral DNS forwarder + reverse-tunnel SaaS in this repo. Any agent
working in `fleetingdns` reads, updates, and cross-links the wiki as part
of normal work — the goal is that we never re-learn the same protocol-layer
or wiring failure twice (cf. the reverse-tunnel postmortem, which is exactly
the class of memory this wiki exists to keep).

Pattern: [Karpathy — LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f).
This file is the local instantiation; the cylon-local-infra wiki (on the
operator's Mac at `~/Workspace/local/cylon-local-infra/llmwiki/AGENTS.md`)
is the sibling we follow stylistically.

## Workspace topology (important)

This repo lives **on ms02** (`~/Workspace/microscaler/fleetingdns`),
along with all other microscaler repos — everything was moved off the
Mac due to disk-space limits. From the operator's Mac the same files are
visible over NFS at `~/Workspace/remote/microscaler/fleetingdns`. The
single exception is `cylon-local-infra`, which remains **local on the
Mac** at `~/Workspace/local/cylon-local-infra`. Consequently:

- Relative cross-repo links to the cylon-local-infra wiki are
  impossible; cite it by Mac-local path in prose instead.
- Any agent editing this wiki over NFS is writing to ms02's disk.
- Tilt/kind/dev workflows run on ms02 natively; the Mac is just a
  viewer + SSH client (see
  [tilt-remote-host-pattern](./concepts/tilt-remote-host-pattern.md)).

## Directory layout

```
llmwiki/
  AGENTS.md          # this file (schema + workflows; agents MUST read first)
  index.md           # content catalog, organized by category
  log.md             # append-only chronological log of ingests/runs/queries/lints
  entities/          # named things: services, binaries, crates, hosts, registries
  concepts/          # topic pages: SSH primitives, russh patterns, dev workflows, anti-patterns
  sources/           # one page per ingested external source (URL, paper, in-repo doc)
  runs/              # one page per provisioning / debug / migration / bring-up attempt
  assets/            # images, diagrams, downloaded HTML-to-MD clips, screenshots
```

The wiki is **owned by the LLM**. Operators curate sources and ask
questions; the LLM reads, summarizes, files, cross-references, and keeps
everything current.

## Three layers

1. **Raw sources** — `README.md`, `docs/**`, `tasks/**`, `Cargo.toml`,
   `crates/**/src/**`, `cmd/**/src/**`, `Tiltfile`, `Justfile`,
   `kind-config.yaml`, `k8s-tilt/**`, `gitops/**`, plus external URLs
   (Karpathy gist, RFC 4254, russh docs, Tilt docs). Immutable.
2. **The wiki** — everything under `llmwiki/`. LLM-written, LLM-maintained.
3. **This schema (`AGENTS.md`)** — conventions + workflows. Co-evolves.

## Page conventions

Every wiki page starts with YAML frontmatter so Obsidian + Dataview (and
plain `grep`) work:

```yaml
---
title: <short title>
kind: entity | concept | source | run
status: active | stale | superseded | draft
tags: [edgehub, edf-cli, dnsd, ssh, russh, tilt, kind, ms02, redis, mtls, ca]
updated: 2026-04-20
sources: [sources/postmortem-reverse-tunnel.md, docs/engineering/POSTMORTEM-reverse-tunnel-connectivity.md]
related: [entities/edgehub.md, concepts/ssh-reverse-tunnel-protocol.md]
---
```

**Filenames**: kebab-case, no spaces. Entities are singular nouns
(`entities/edgehub.md`, `entities/edf-cli.md`). Concepts describe one idea
(`concepts/ssh-reverse-tunnel-protocol.md`).

**Links**: always relative markdown links so they work in Obsidian and on
GitHub. Cross-repo references to the cylon-local-infra wiki cannot be
relative links (it lives on a different machine — see "Workspace
topology" above); cite it by Mac-local path in prose.

**Code citations**: use ``code-fence`` references with `crate/file.rs:line`
form, e.g. `` `crates/edgehub/src/ssh_server.rs:1075` ``. Do **not** paste
large code blocks; the wiki is a *notebook*, not a fork of the source.

**Evidence > opinion**: every claim that could be wrong carries a citation
to a source page, a commit, or a dated `runs/<slug>.md` entry.

## Workflows (the LLM follows these)

### Ingest

Triggered when a new source is added (URL, in-repo doc, RFC, upstream
issue, chat excerpt) or an ad-hoc observation surfaces during a session.

1. Read the source. Summarize at the top of `sources/<slug>.md` (≤ 10
   bullet points, frontmatter `kind: source`).
2. Extract entities and concepts referenced. If any lack a page, create
   minimal stubs with frontmatter `status: draft`.
3. Update existing entity / concept pages. When adding a new claim that
   **contradicts** an existing one, do **not** silently overwrite — add a
   `## Contradictions` section, keep both claims with dates + citations,
   and mark the older one `status: superseded` if clearly wrong.
4. Add a one-line entry to `index.md` under the right category.
5. Append to `log.md` with today's date and the consistent prefix:
   `## [YYYY-MM-DD] ingest | <source title>` — one paragraph describing
   what changed.

### Run (debug / migration / bring-up / fix attempt)

Every meaningful execution gets a page under `runs/`.

1. Create `runs/YYYY-MM-DD-<short-slug>.md` with frontmatter `kind: run`.
2. Record: command(s) issued, host(s), git SHA, wall-clock, outcome
   (**success** / **partial** / **failure**), and a "what worked / what
   did not" table.
3. If the run revealed a new failure mode, create or update the relevant
   `concepts/<name>.md` page and link from the run page. **This is how we
   stop forgetting.**
4. Append a `## [YYYY-MM-DD] run | <slug> | <outcome>` line to `log.md`.

### Query

1. Read `index.md` to find relevant pages.
2. Read those pages; drill into sources if needed.
3. Answer with citations (markdown links back into `llmwiki/`).
4. If the answer is non-trivial and likely to be asked again, **file it
   back** as a new concept page or append to an existing one. Chat
   history is not memory — the wiki is.

### Lint (periodic health check)

Run when `log.md` has grown by ~20 entries or on operator request:

- Flag pages with `status: active` but `updated` older than 30 days that
  cover areas with recent commits — surface as "stale?".
- Flag orphans: pages with no inbound links. Either link them in or delete.
- Flag concepts referenced in ≥2 pages but lacking their own page —
  propose creation.
- Surface `Contradictions` sections and ask operator to adjudicate.
- Output a markdown report; **do not** auto-delete pages.

## Conventions for this repo specifically

- **Components live in `crates/` and `cmd/`** (Rust workspace). Each
  workspace member that ships a binary or is referenced by a service gets
  an entity page.
- **Dev cluster lives on ms02** (shared with cylon-local-infra). Tilt runs
  ON ms02; the Mac SSHes in with a -L forward for the Tilt UI port. See
  [tilt-remote-host-pattern](./concepts/tilt-remote-host-pattern.md).
- **No shell scripts.** Operator-facing automation goes in `scripts/*.py`
  invoked from the `Justfile`. Mirror of the cylon-local-infra rule.
- **Debug sessions** live in `.cursor/debug-<id>.log` (NDJSON). When a
  page cites runtime evidence, include the session id and approximate
  timestamp.
- **Dates in EEST/UK**. Use ISO-8601 `YYYY-MM-DD` in filenames and log
  prefixes.
- **No secrets** in the wiki. Keys, tokens, passwords live in `.env` /
  sops. If a journal excerpt contains a token, redact before filing.

## Anti-patterns (do not do)

- Don't write narrative essays. Wiki pages are reference material; keep
  them dense.
- Don't paraphrase `docs/` when you can link to it. `docs/` is source of
  truth for product/architecture; the wiki is the **notebook** that
  records what worked when you ran it and what broke when you tried.
- Don't delete failed runs from `runs/`. The wiki's job is to remember
  the failures.
- Don't commit `Co-authored-by: Cursor …` — clients prohibit it (also a
  user-rule in this workspace).

## Bootstrapping order (new agent, new session)

1. Read this file.
2. Skim `index.md`.
3. `tail -30 log.md` to see what happened recently.
4. Read the entity/concept pages most relevant to the task before acting.
5. If you're touching the SSH/tunnel data plane, read
   [postmortem-reverse-tunnel](./sources/postmortem-reverse-tunnel.md) +
   [ssh-reverse-tunnel-protocol](./concepts/ssh-reverse-tunnel-protocol.md)
   first. **Do not** re-introduce `direct-tcpip` for reverse forwarding.
