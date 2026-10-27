# Relay API

**Version:** 1
**Status:** normative for v0.1

The relay stores and forwards opaque blobs. It does not parse events, membership, or identities. A conforming relay can be written from this document alone.

## Mailboxes

A mailbox id is 32 bytes from a CSPRNG. Clients MUST NOT derive it from a name, a key, or an identity. Possession of the id is the only write and read capability. There is no account, session, or device authentication.

Clients learn mailbox ids from pairing offers and from sealed membership events ([02-identity-log.md](02-identity-log.md), [03-spaces.md](03-spaces.md)). Rotating an inbox is those events, not a relay operation.

The relay MUST treat the id as an opaque key. It MUST NOT require, or provide a field for, a user id.

## Transport

Clients speak newline-delimited JSON over a WebSocket. The relay MUST be reached over TLS. Cleartext WebSocket is not a conforming deployment.

Request and response objects have an `op` string. Unknown ops return `{"op":"err","code":"bad_request"}`. Binary fields are unpadded base64url.

Limits:

- A blob larger than 4194304 bytes is `too_large` and MUST NOT be stored.
- `limit` on a get defaults to 100 and MUST be capped at 500 by the relay.

## Operations

Put:

```json
{"op":"put","mailbox":"<base64url>","blob":"<base64url>"}
```

Response: `{"op":"ok","cursor":1}`. The cursor is a positive integer assigned by the relay, strictly increasing per mailbox. It is not a Space order. Clients MUST NOT use it to merge events.

Get:

```json
{"op":"get","mailbox":"<base64url>","since":0,"limit":100}
```

`since` is exclusive. An unknown mailbox returns an empty list, the same as an empty one.

```json
{"op":"blobs","items":[{"cursor":1,"blob":"<base64url>"}]}
```

Delete:

```json
{"op":"delete","mailbox":"<base64url>","up_to":1}
```

Deletes stored blobs in that mailbox with cursor less than or equal to `up_to`. Response is `{"op":"ok"}`. Delete is best-effort storage cleanup. It is not un-sending.

Subscribe:

```json
{"op":"subscribe","mailbox":"<base64url>","since":0}
```

The relay sends `blobs` objects as new puts arrive, using the same item shape. A client that only polls with get is conforming. Subscribe is optional for the client and required for the relay.

## What the relay must not do

- Decrypt, or inspect a blob for a type, a key, or a Space id.
- Link two mailboxes because the same IP used both. It will see the IPs. It MUST NOT be given a protocol field that makes the link for it.
- Log blob bytes, mailbox ids, or source addresses in application logs. Connection errors MAY be counted without those values.

Retention, quotas, and rate limits are deployment policy. A self-hosted relay MAY expire blobs 30 days after the put. Expiry is not a security boundary.

## Errors

`bad_request`, `too_large`. No `unauthorized`. A wrong mailbox id is indistinguishable from an empty mailbox.

## Changelog

- v1 (2026-10-27): put, get, delete, subscribe. Mailbox id is the capability.
