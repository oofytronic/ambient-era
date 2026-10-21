# Security policy

**Status: pre-0.1 and unaudited.** Nothing in this repository should be relied on to protect real secrets yet. An independent crypto and protocol review is planned before v1.0 (see the roadmap in the [README](README.md)).

## Reporting a vulnerability

Please report it privately through GitHub: the repository's **Security** tab, then **Report a vulnerability**. Do not open a public issue for a security problem.

Include what you found, how to reproduce it, and which version or commit. This is a small project and responses are best-effort, but reports are read and taken seriously.

## Scope

In scope: the protocol, the Rust core crates, the relay, and the client apps in this repository. Out of scope: third-party libraries (report those upstream) and attacks that need a compromised device.

## What we promise, and what we don't

The security tiers and their limits are described in [VISION.md](VISION.md) section 3. We do not claim anonymity until network anonymity (Tor mode) exists, and we do not claim that shared content cannot be copied by a recipient.
