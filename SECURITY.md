# Security policy

**Status: pre-0.1 and unaudited.** Nothing here should be trusted with a real secret yet. An independent review is planned before v1.0. The promises that review has to measure are in [THREAT_MODEL.md](THREAT_MODEL.md), not in this file.

## Reporting a vulnerability

Report it privately through GitHub: the repository's **Security** tab, then **Report a vulnerability**. Do not open a public issue for a security problem.

Include what you found, how to reproduce it, and which version or commit. This is a small project and responses are best-effort, but reports are read.

## Scope

In scope: the specs, the Rust core, the relay, and the client apps in this repository.

Out of scope:

- Third-party libraries. Report those upstream.
- A stolen device that is unlocked, or whose keystore is extracted. v0.1 does not promise forward secrecy. See the threat model.
- Someone who already decrypted an event and kept a copy.
- Traffic analysis and IP collection. Hiding the IP is the later Tor mode, not v0.1.

## Rules for changes

- Do not add a cipher, a ratchet, or a key schedule. HPKE (RFC 9180) to member device keys is the private-Space construction ([ADR-0005](docs/adr/0005-private-space-encryption.md)).
- Do not log plaintext, keys, mailbox ids, or tokens.
- Do not describe the product as anonymous. Do not describe revocation as deletion.
- Crypto and identity changes need owner review (see `AGENTS.md`).
