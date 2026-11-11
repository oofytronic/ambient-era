# Connection

The network track inside this repo. The notebook is the app. This is how a device reaches another device.

The protocol of a network is already here: an identity, a Space, sealed logs, and a mailbox id that is the only capability. What is not here is a connection. The relay on this computer is cleartext, in memory, and the window only dials itself. Two folders on one machine is not a network.

This track is that connection. It is not a second app, not a second membership model, and not a directory of people. Spec 04 stays the wire. [ADR-0005](../adr/0005-private-space-encryption.md) stays the seal. A byte change still starts in the spec, not here.

## What a person should see

They open the app. They never type an address. Notes and a 1:1 reach their other computer, and one other person, including after the first relay is gone.

If every relay is down, the notebook still works on this computer. A checkmark means the note is saved here. The globe stays gray when nothing is connected, red when a relay is up and something has not been sent, and green when it is caught up. The relay only holds copies of sealed blobs.

Advanced settings are the only place an address appears: use only this relay, add a backup, or stay on this computer. Running your own relay is the advanced case. It is the same protocol.

## What this is allowed to claim

A small introduction list, run by the owner at first, is a convenience. It is not an authority. The honest claim is:

- The operator cannot read the blobs, and is not given a user id to link them.
- The notebook does not live on the relay. A relay that dies is replaced by another, and the device sends again from its own log.
- There is no account to be removed from. Losing the introduction list does not delete notes.
- The product is not anonymous. Hiding the IP is a later mode, not this one.

Do not claim that no center exists while the introduction list is a handful of addresses the owner operates. Claim that the center can be swapped, and that swapping it is not a migration.

## What this is not

- A login, an account, or a global user id.
- A directory. People still meet by a link or a QR.
- Relays talking to each other. A client writes to a relay it chose and reads from one. There is no federation protocol in this MVP.
- Tor, direct peer connections, public mirrors, payments, or a public feed. Those stay later rows of the same track, not new products.
- A promise that a blob, once accepted, is forwarded even if this process forgot it. Durability is a requirement below, not a property of today's memory store.

## MVP

Build in this order. Do not start the later rows to make the demo more impressive.

| Step | Work | Done when |
|:--|:--|:--|
| 1 | This charter | The other docs point here. Spec 04 is unchanged. |
| 2 | Durable mailboxes | Done for this computer. `ambient-relay` writes `mailboxes` in a directory and refuses a file it cannot read, so a bad read is not saved back over the real mail. A restart keeps the blobs and does not reuse a cursor. The window's file is `~/.local/share/ambient/relay/mailboxes` (`$XDG_DATA_HOME/ambient/relay` when that is set). The wire is unchanged. Still loopback, still cleartext. |
| 3 | An address the client can hold | Done. The folder file `relays` remembers this computer, the introduction list, or a pin plus a backup. Loopback is the default, not the only path. `ws://` is refused for any host that is not this computer. |
| 4 | TLS off the machine | Done in the program. The client speaks `wss` with rustls and the webpki root set. The relay terminates TLS when started with `--tls` and a PEM the operator already has. Loopback stays cleartext. No certificate is requested by this program. |
| 5 | Introduction list, and send again | The list is a constant and is empty until a real relay exists. The client tries the remembered relay, then the list, and on pin mode the backup. A blob accepted by one relay is sent again to the next. Cursors are per relay. If every relay fails, the note stays saved and the checkmark says so. |
| 6 | Advanced, not the first screen | Done. The address fields are behind Advanced on the globe card. This computer, a pin, and a backup. No address on the notes screen. |
| 7 | The replacement test | One relay from the list is up, with TLS and a disk. A second computer, with no address typed, receives a note and a chat line. Stop that relay. The next one on the list receives a re-send. The note arrives again. The first operator never saw plaintext. |

**Exit.** The owner uses a second computer for real notes and one 1:1, with one relay turned off in the middle, and does not type an address to recover. A green test is not the exit.

Polling with get is enough. Subscribe stays required for a conforming relay and optional for the client.

## Running a relay

The program is `ambient-relay` in this repo. There is no package to install and no image to flash.

The notes window already starts one on this computer. Restarting the window does not empty its mail. That relay still only listens on this machine, in the clear. It is not the introduction list.

A relay another computer can reach is the same program, on a machine that stays on, given a directory:

```
ambient-relay 8787 /var/lib/ambient-relay
```

That form is cleartext and only binds this computer. A relay another computer can reach is the same program with TLS, and a PEM certificate the operator already has:

```
ambient-relay --tls fullchain.pem privkey.pem --store /var/lib/ambient-relay --bind 0.0.0.0:443
```

This program does not request a certificate. A small VPS is the straightforward machine. A computer at home is the same binary, if the port can be reached and the machine stays up. Neither is a special image. Do not rent one until there is a certificate and the address is put in the introduction list or pinned. Until then the app stays on this computer.

## After the MVP, still this track

Not permission to start.

| Later | What changes | What it must not become |
|:--|:--|:--|
| More than one relay at once | The client writes to more than one, so one outage needs no re-send | A relay-to-relay protocol |
| One-command self-host | The advanced path is a command, not a guide. This is the v0.5 row | A hosted service as the only way to sync |
| Paid operators | Someone who wants a relay pays the operator. Reputation is uptime | A token. An account. An obligation to run a server at home |
| Bootstrap that can die | The introduction list can be replaced out of band. The protocol works after it is gone | A directory that is the only way in |
| Direct connections, then Tor | Behind the transport trait, when both peers are reachable. Anonymity is not claimed before Tor ships | A rewrite of the mailbox |

## Where to change something

| Change | Touch |
|:--|:--|
| A byte on the wire | [spec 04](../spec/04-relay-api.md), then a vector, then code |
| How a mailbox is stored | `ambient-relay`. It still must not parse blobs |
| Which relay a folder uses | The client in `ambient-cli`, then the window. Not a new protocol |
| What the globe is allowed to say | [THREAT_MODEL.md](../../THREAT_MODEL.md) first |
