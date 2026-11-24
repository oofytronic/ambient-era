# Standing up a relay

Not started. The owner is not doing this yet. This is the path for when a second computer is in reach. [README.md](README.md) is the track. The window's own relay is not this. It listens on this computer, in the clear, and another machine cannot see it.

A certificate is a file that says a name is this machine, signed by someone both computers already trust. Ambient trusts the public list (the webpki roots) and will not accept a certificate you make yourself. This program does not ask for one. Let's Encrypt gives one for free. It lasts ninety days.

The certificate is for a name, such as `relay.example.com`, not for the machine and not for a number. Both notebooks pin `wss://` that name.

## A computer at home may not be reachable

A Raspberry Pi works only if the internet can open a port on it. Look at the address the router was given by the provider. If it starts with `10.`, `192.168.`, or `100.64.`, the house is behind another network and a machine in it cannot be a relay. A small VPS is the straightforward machine then. It already has a public address. The steps below are the same, with no router in the way.

## The steps

1. Own a name. At the place it was bought, add a record named `relay` that points at the public address. That address changes. When the notebooks stop connecting, check the record first.

2. Forward one port from the router to the machine, `8443`. Leave `80` open until the certificate is issued, then close it. Let's Encrypt uses `80` once, to prove the name is yours.

3. Ask for the certificate. On the machine:

```
sudo apt update
sudo apt install certbot
sudo certbot certonly --standalone -d relay.example.com
```

It writes two files. Do not copy the key anywhere else.

- `/etc/letsencrypt/live/relay.example.com/fullchain.pem`
- `/etc/letsencrypt/live/relay.example.com/privkey.pem`

4. Build the relay there. It is not a package. Install Rust, clone this repo, and from `ambient-era` run `cargo build -p ambient-relay --release`. A Pi takes a while. The program is `target/release/ambient-relay`.

5. Start it on `8443`, so it does not have to run as root. Create the directory first and let the user who runs it own the directory.

```
ambient-relay --tls /etc/letsencrypt/live/relay.example.com/fullchain.pem \
  /etc/letsencrypt/live/relay.example.com/privkey.pem \
  --store /var/lib/ambient-relay --bind 0.0.0.0:8443
```

6. On each computer, open the globe, then Advanced. Pinned relay is `wss://relay.example.com:8443`. Save it. The globe goes green when that machine answers.

`sudo certbot renew` replaces the files. The relay has to be started again afterwards, or it keeps the old certificate in memory.
