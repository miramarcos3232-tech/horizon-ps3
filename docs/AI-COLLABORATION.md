# AI Collaboration Protocol

This repository may be reviewed by multiple assistants and human contributors. This document defines a safe, reviewable workflow.

## Important capability note

Claude and Manus are not currently connected to this ChatGPT workspace through a confirmed direct repository integration. Do not assume either agent can read or write this repository until its own GitHub authorization is explicitly configured and tested.

## Branch and pull-request rules

- Never push experimental work directly to `main`.
- Each contributor works on a separate branch named `agent/<name>/<topic>` or `work/<topic>`.
- Open a pull request targeting `main` for every proposed change.
- Do not merge automatically. A human maintainer reviews the diff and test evidence first.
- Do not overwrite another contributor's branch or force-push shared work.
- Keep changes small and focused; explain the reason for each change.

## Required pull-request report

Every PR must include:
1. Goal and scope.
2. Files changed.
3. Exact commands run.
4. Test results, including failures.
5. Whether tests were run on a host, emulator, or real PS3.
6. Known limitations and follow-up work.

Never claim a build, test, emulator run, or PS3 hardware test succeeded unless it was actually executed and its output is available.

## Initial technical milestone: Rust PS3 smoke test

The first milestone is a minimal Rust executable using the existing PS3 toolchain/SDK. It is not yet a Horizon game port.

Acceptance criteria:
- Pin the exact Rust toolchain and SDK revisions.
- Build a minimal program for `powerpc64-sony-ps3`.
- Run the documented ELF post-processing step, if required by the selected SDK.
- Inspect ELF architecture, entry point, sections, and unresolved symbols.
- Record the exact build output and artifacts.
- Only claim runtime success after running in RPCS3 or on real PS3 hardware.

## Technical guardrails

- Verify upstream repository state and license before vendoring or copying code.
- Do not invent POSIX or `std` implementations merely to make compilation pass.
- Prefer a minimal `no_std` + `alloc` smoke test before attempting a custom `std` PAL.
- Keep platform-specific code behind a narrow interface so Horizon integration can be evaluated separately.
- Preserve upstream attribution and license notices.

## Suggested division of work

- ChatGPT: coordinate milestones, inspect upstream sources, integrate reviewed changes, maintain project documentation.
- Claude (if separately connected by the owner): independent Rust/code review and architecture critique.
- Manus (if separately connected by the owner): research tasks, upstream dependency/license checks, and reproducible investigation reports.
- Human maintainer: approve repository access, review PRs, and decide merges.

## Security

Grant external agents the minimum repository permissions needed. Prefer read access for review-only work. Do not share personal access tokens, passwords, or private keys in issues, commits, or chat.
