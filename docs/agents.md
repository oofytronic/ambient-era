# Agents

Recorded 2026-11-12. The owner's note, kept as written, is [agent-layer.md](agent-layer.md). This file is how that note is read against the app that exists. It is not a v0.1 feature. Do not scaffold a runtime, a supervisor, or a new record until a spec says the merge rule changed. The v0.1 fence in [ARCHITECTURE.md](../ARCHITECTURE.md) still holds: no running agent.

An agent is a short-lived grant around a model. It is not a teammate, a bot with a memory, or a second user.

## What it is

The model proposes. The runtime disposes. The model never writes state itself and never sees data outside the grant it was given.

- **Identity.** A fresh local key, minted at spawn, signed by the root device key or by a parent that is allowed to spawn. The key lists capabilities and an expiry. It is not a role.
- **Scope.** Every proposed action is checked against that list before it runs. A failed check is logged and dropped.
- **Life.** The agent does the job, writes one signed log entry per mutation, and dies. It does not keep a memory or a personality.
- **Model.** Swappable. A small local model for sorting and drafting. A remote model only when the job needs it. The runtime checks scope either way. A remote call sends only the slice the grant allowed, and that slice leaves the machine.

A parent with spawn rights may mint a narrower child. The parent dies when its job ends. Revocation is local: invalidate the key and later actions fail.

## What lasts

Durable knowledge lives in ordinary local data: notes, tasks, messages, contacts, and the rules and preferences the owner set or that an approved run wrote as data. Later agents can read those. They cannot read an earlier agent's conversation.

The log is append-only, hash-chained, and signed. It is the witness. The notes and messages are still the source of truth. The log has to stay readable without a dashboard. One tap revokes.

Learning means updating a rule a later agent can see. It does not mean the agent remembering.

## Who starts one

A small local supervisor watches triggers: an event, a schedule, a rule, a request. It spawns an agent with the right scope. It does not reason. Multi-step work is a chain of agents handing a structured result forward, or a parent spawning narrower children.

The supervisor is not the notes window. An always-on process is a later program. Do not hang it off the typing thread.

## What the person sees

Most runs produce nothing to look at. Three surfaces, and only when needed:

1. An exception queue, with the context already gathered, for a judgment the rule did not cover.
2. A recent summary, only when asked.
3. The log, searchable, with revocation.

The product should still feel like the notebook and the messenger. Not a console of bots.

Anything that leaves the machine, or that another person will see, waits for approval unless the owner already wrote a rule that allows that exact kind of action. Editing a note the owner is looking at is not the same as sending a chat.

## Shared rooms

A notebook is the owner's devices. An agent there is the owner's grant.

A conversation has another member. The existing rule stands: an agent does not read a shared Space unless every member has agreed. It may draft from the owner's own side. It does not get the other person's keys by being helpful.

The grant is the capability in [ADR-0001](adr/0001-creations-as-spaces.md). It is not a new protocol. A new record is justified only when the merge rule is different. The log is that candidate, and it waits for a spec.

## Order, when it is time

1. Capability language on the device keys that already exist. Time bounds included.
2. The signed local log.
3. The supervisor.
4. The model adapter.
5. First jobs on notes and tasks: draft, remind, apply only when a rule already allows it.
6. Chat, still only with the other member's agreement before a shared read or a send.
7. Sync of state and the log on the existing end-to-end path. Signed mirrors for discovery. No permanent directory.
8. Approvals, corrections, and ignores write rules. They do not train a private memory.

## What this is not

- Not Zapier. A script breaks when the input was not anticipated. The model may choose a path inside the grant. The grant does not grow because the model asked.
- Not a long-lived assistant with its own memory, skills, or a presence on other networks.
- Not a company workspace, a CRM, or a team memory. The person is a principal, not a manager of agents.
- Not standing power, and not a model trusted because it said it would ask.

## Open

- Which actions are pre-approved by default, and which always wait. A note draft is a different risk from a sent message.
- A remote model sees the slice. The threat model has to say that plainly before the adapter exists.
- The log's event shape is not specified. Do not invent the bytes in the desktop crate.
