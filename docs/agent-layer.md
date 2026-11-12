# Ambient Agent Layer

Recorded 2026-11-12. The body below is the owner's note, kept as written.

[agents.md](agents.md) is how to read this note while building. It does not start a runtime.

Where this note disagrees with an ADR, [ARCHITECTURE.md](../ARCHITECTURE.md), [ROADMAP.md](../ROADMAP.md), or [THREAT_MODEL.md](../THREAT_MODEL.md), those win. Two bindings are already set:

- This is not v0.1. Do not scaffold a supervisor, a log format, or a model adapter in the desktop crate.
- An agent does not read a shared Space unless every member has agreed. "Chat-scoped" means that. It does not mean a private read of the other person.

The note supersedes the earlier agent framing in [VISION.md](../VISION.md) §11. A remote model may run when the job needs it, and only the granted slice leaves the machine. The threat model has to say that before an adapter exists.

---

## Purpose of This Document

This document captures the agreed vision, architectural decisions, and strategic positioning for adding a short-lived, local-first, scoped agent layer to the Ambient everything app. It is intended as durable context for future agents, collaborators, and implementers.

Ambient currently consists of a local-first, end-to-end encrypted notebook and 1:1 messenger with notes, chat, and a task list. The goal is to evolve it into an ambient system in which most digital work happens quietly in the background, and the user interacts primarily by approving, correcting, or ignoring exceptions.

## Core Vision

The product should feel like quiet infrastructure for digital life rather than software the user operates or a company the user manages. Agents perform routine, low-judgment, and moderately ambiguous work in the background. The user is interrupted only when human judgment is actually required. Interaction collapses to feedback: approve, correct, or ignore.

This is deliberately not a persistent multi-agent team, a self-improving personal assistant with long-term memory of its own, or a dashboard of bots. It is an ambient layer whose orchestration is simple automation and whose individual units of work are short-lived agents.

## What an Agent Is (and Is Not)

An agent is the identity, scope, and runtime harness around a model — not the model itself.

- **Identity**: A fresh local keypair minted at spawn time, signed by the user’s root device key (or by a parent agent that holds spawn rights). The key carries a narrow capability list and an expiry.
- **Runtime**: Enforces the scope. The model proposes structured actions; the runtime checks every proposal against the capability list, executes only what passes, and writes a signed log entry. The model never touches state directly and never sees data outside its scope.
- **Model**: A swappable inference engine. Local small models handle high-frequency glue work (classification, routing, drafting). Remote models handle harder judgment. The runtime remains model-agnostic.

Agents are ephemeral. They spawn for a job, act within their scope, write a signed log entry, and die. They do not retain their own memory or personality. Durable knowledge lives in ordinary local data (notes, tasks, messages, rules, preferences) and in the signed audit log.

### Distinction from Existing Systems

- **Classic automation (Zapier, cron, scripts)**: Fully pre-specified paths. No open-ended reasoning at runtime. Breaks when inputs deviate from what was anticipated. The user remains the workflow engineer.
- **Hermes Agent (Nous Research)**: Long-running, self-hosted, persistent memory, self-improving skills, multi-surface presence (Telegram, Discord, Slack, etc.). The agent grows with the user over time.
- **Grok Bot**: Long-lived agents sharing a cloud computer, with memory and routines, able to act in connected services while the user is away.
- **This design**: Orchestration is dumb (a supervisor matches triggers to spawns). Work units are short-lived agents that can handle novel or ambiguous situations via a model. Authority is scoped and time-limited. The user is a principal who sets rules and reviews exceptions, not a manager of agents.

The agentic quality comes from the model’s ability to interpret context, choose tools, and navigate unmapped paths within a hard scope. The ambient quality comes from the fact that most runs never surface.

## Key Architectural Components

### Identity and Delegation

- Root key lives on the user’s primary device.
- Agents receive derived, short-lived keys with explicit capability lists (not roles). Example: “read unread messages in this channel, write one draft, expire in 30 minutes.”
- An agent with spawn rights can mint narrower children.
- Revocation is local and immediate: invalidate the key and future actions fail. No remote session to kill.
- Bootstrap uses ordinary public-key exchange between parties who already trust each other out-of-band. No permanent central directory.

### Capability Enforcement and Runtime

- Every proposed action is checked against the capability list before execution.
- Failed checks are logged and dropped.
- The scope checker is the primary trust boundary.
- Design rule: the model proposes; the runtime disposes.

### Signed Local Audit Log

- Append-only, hash-chained, signed entries (one per mutation).
- Tamper-evident: editing an entry breaks the chain.
- Lives on the user’s machine. The log is the witness; application state is the source of truth.
- The user can search the timeline and revoke with one tap.
- The log must remain readable without a dashboard.

### Supervisor

- Lightweight, always-running local process.
- Watches triggers (events, schedules, rules, one-off requests) and spawns short-lived agents with the correct scope.
- Does not itself reason.
- Multi-step work is expressed as chains of agents that hand structured results forward, or as a parent that spawns narrower children. The parent dies when its job ends.

### State and Memory

- No persistent agent memory.
- Durable knowledge lives in ordinary local stores (notes, tasks, messages, contacts, calendar) plus explicit rules and preferences the user sets or that approved runs write as structured data.
- Agents receive a tightly scoped view of relevant data for their lifetime only.
- Learning happens by updating rules and preferences that later agents can see, not by agents retaining conversation history.

### Model Adapter

- Thin, swappable interface.
- Local models (e.g., via Ollama) for routine high-frequency work.
- Remote models for deeper reasoning.
- The runtime enforces scope regardless of which model is used.

### Sync and Discovery

- Existing local-first E2E layer carries encrypted state and the log.
- Signed mirrors (cheap VPS, IPFS, or similar) allow other devices to find sleeping nodes without a permanent central directory.
- Optional paid relays provide connectivity for users who do not self-host. Relays never see plaintext state or hold long-lived authority.

## Ambient User Experience

The interface is mostly quiet. Three primary surfaces:

1. A quiet exception queue: items that require human judgment, with context already assembled.
2. An optional recent-activity summary (only when requested).
3. The full searchable signed log, with one-tap revocation.

Most agent runs produce no notification. The user experiences digital life as things that simply get done, with rare, high-signal interruptions.

## Mapping from the Current App

Current foundation: local-first E2E notebook/messenger with notes, 1:1 chat, and task list.

Recommended build order:

1. Identity and capability system on top of existing device keys. Define a simple capability language with time bounds.
2. Signed append-only local log for agent actions.
3. Lightweight supervisor that watches triggers and spawns scoped agents.
4. Model adapter (local + remote).
5. First agent surfaces on tasks and notes (read relevant data, draft updates or reminders, apply if pre-approved or surface for approval).
6. Chat-scoped agents.
7. Multi-device sync of state and logs via the existing E2E layer; signed mirrors for discovery.
8. Feedback loop: approvals, corrections, and ignores update durable rules and preferences.

The product should continue to feel like the existing notebook and messenger that quietly does more, not a new agent-management console.

## Ultimate Trajectory

- **App**: Reference client that delivers the ambient experience.
- **Network**: User-owned devices plus optional paid relays, mirrors, and compute. No permanent central directory. Authority remains with the user’s keys.
- **Company**: Provides optional infrastructure (relays, model routing, recovery tools, mirrors) and the reference client. Does not become the identity or data custodian.
- **Hardware** (optional, later): Devices that make on-device sensing, local inference, or always-available mirrors easier (home servers, unobtrusive wearables). Not required for the vision.

## Strategy Against Platform Giants

Apple, Meta, and others can ship polished, highly accessible versions of “AI that does things in the background.” They cannot easily ship the combination of:

- True local-first operation with user-controlled keys and no permanent central directory.
- Short-lived scoped authority that dies with the task.
- A local signed audit log the user can read and revoke.
- An economic model for paid operators that does not require those operators to hold standing access or become the identity provider.

Their distribution and polish advantages become liabilities if the valuable property is the absence of standing platform trust. The wedge is users who already feel the constraints of platform custody (power users, privacy-oriented builders, and people who have watched platforms change rules after investment).

Make the protocol and reference client strong enough that a convenient version without the trust model feels incomplete. Open the core so the alternative continues to exist even if a giant ships a more accessible surface. The goal is not to out-distribute on day one; it is to own the architecture that makes the ambient, user-owned version possible.

## Feasibility Notes

The core agent loop (observe → model proposes → runtime checks scope → execute → log → die) is not especially hard. Many minimal implementations exist. The design is simpler than persistent multi-surface agents because agents are ephemeral and do not accumulate their own memory.

The harder work is the surrounding system the project already prioritizes: reliable local state, E2E multi-device sync, signed local audit, quiet failure modes, and an interface that stays out of the way. A usable first version operating on notes, tasks, and local chat is achievable. Full ambient reliability is a longer project, but the architectural bet is narrower than general-purpose agent platforms.

## Summary Constraints for Implementers

- Agents start with nothing and receive only short-lived scoped authority.
- The model proposes; the runtime disposes.
- All durable knowledge lives in ordinary local data and the signed log, not inside agents.
- The user is interrupted only for judgment.
- No permanent central directory.
- Local-first and E2E remain non-negotiable.
- The system should feel like quiet infrastructure, not a company or a dashboard.

This document supersedes earlier exploratory framing. When in doubt, prefer the constraints above.
