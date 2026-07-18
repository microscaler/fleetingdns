---
title: Karpathy — LLM Wiki gist
kind: source
status: active
tags: [llmwiki, blueprint, karpathy]
updated: 2026-04-20
url: https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f
related:
  - AGENTS.md
---

# Karpathy — LLM Wiki gist

Original source describing the persistent-wiki pattern that this
`llmwiki/` directory instantiates.

## Summary (≤ 10 bullets)

- LLMs commonly do RAG: retrieve fragments per query, re-derive
  knowledge each time, never accumulate. The wiki idea is the inverse —
  the LLM **incrementally builds and maintains a persistent wiki** of
  markdown files between you and your raw sources.
- Three layers: (1) raw sources (immutable), (2) the wiki (LLM-written),
  (3) the schema document (`AGENTS.md` / `CLAUDE.md` — tells the LLM how
  to maintain the wiki).
- Three operations: **ingest** (file new sources, update entity/concept
  pages, log it), **query** (answer from the wiki, file good answers
  back), **lint** (periodic health check for stale claims, orphans,
  contradictions).
- Two special files: `index.md` (content catalog by category) and
  `log.md` (chronological, append-only — `## [YYYY-MM-DD] <op> | <slug>`
  prefix makes it greppable).
- Filenames are kebab-case markdown; cross-links are relative; YAML
  frontmatter (`title`, `kind`, `status`, `tags`, `updated`, `sources`,
  `related`) lets Obsidian + Dataview do useful things.
- Operator's job: curate sources, ask questions, decide what matters.
  LLM's job: summarize, cross-reference, file, maintain consistency.
- Optional CLI tooling like `qmd` (BM25 + vector search over local
  markdown) helps as the wiki grows; the index file alone is enough at
  small scale.
- Use cases: research, business/team wiki, reading a book, personal
  journals, due diligence — anywhere knowledge accumulates over time.
- The wiki is a git repo of markdown files → free version history,
  branching, collaboration.
- Spiritually related to Vannevar Bush's Memex (1945): private,
  curated, with associative trails. The piece Bush couldn't solve was
  *who does the maintenance*. LLMs do.

## Why we adopted it for fleetingdns

The reverse-tunnel postmortem identified that the same protocol-layer
mistake survived 15+ PRs because nobody could see the cumulative
context. A persistent, LLM-maintained wiki is precisely the artifact
that would have surfaced the contradiction between
[ssh-reverse-tunnel-protocol](../concepts/ssh-reverse-tunnel-protocol.md)
(what the PRD requires) and
[edgehub](../entities/edgehub.md) (what the code does) on PR #52.

## See also

- [`AGENTS.md`](../AGENTS.md) — fleetingdns instantiation of the schema.
- The cylon-local-infra wiki's version of this same source lives on the
  operator's Mac at
  `~/Workspace/local/cylon-local-infra/llmwiki/sources/karpathy-llm-wiki.md`
  (different machine — no relative link).
