#!/usr/bin/env python3
"""Generate Ambient Era v1 conformance vectors.

Run from anywhere. Requires pynacl and blake3.

The HPKE section checks itself against CFRG RFC 9180 test vector
(base mode, DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20Poly1305)
before writing. Event bytes come from the encoder below. They are
provisional until a second implementation reproduces them.
"""

import hashlib
import hmac
import json
import os
import struct

from blake3 import blake3
from nacl.bindings import (
    crypto_aead_chacha20poly1305_ietf_encrypt,
    crypto_scalarmult,
    crypto_scalarmult_base,
)
from nacl.signing import SigningKey

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
VEC = os.path.join(ROOT, "conformance", "vectors")


def hx(b: bytes) -> str:
    return b.hex()


def unhex(s: str) -> bytes:
    return bytes.fromhex(s)


def hkdf_extract(salt: bytes, ikm: bytes) -> bytes:
    if not salt:
        salt = b"\x00" * 32
    return hmac.new(salt, ikm, hashlib.sha256).digest()


def hkdf_expand(prk: bytes, info: bytes, length: int) -> bytes:
    out = b""
    block = b""
    counter = 1
    while len(out) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        out += block
        counter += 1
    return out[:length]


def hkdf(ikm: bytes, salt: bytes, info: bytes, length: int) -> bytes:
    return hkdf_expand(hkdf_extract(salt, ikm), info, length)


def i2osp(n: int, w: int) -> bytes:
    return n.to_bytes(w, "big")


def encode_uint(n: int) -> bytes:
    if n < 0:
        raise ValueError("negative integers are not used")
    if n < 24:
        return bytes([n])
    if n < 256:
        return bytes([24, n])
    if n < 65536:
        return bytes([25]) + struct.pack(">H", n)
    if n < 2**32:
        return bytes([26]) + struct.pack(">I", n)
    return bytes([27]) + struct.pack(">Q", n)


def head(major: int, length: int) -> bytes:
    return bytes([(major << 5) | encode_uint(length)[0]]) + encode_uint(length)[1:]


def cbor_bstr(b: bytes) -> bytes:
    return head(2, len(b)) + b


def cbor_tstr(s: str) -> bytes:
    raw = s.encode("utf-8")
    return head(3, len(raw)) + raw


def cbor(v):
    if v is None:
        return b"\xf6"
    if isinstance(v, bool):
        return b"\xf5" if v else b"\xf4"
    if isinstance(v, int):
        return encode_uint(v)
    if isinstance(v, (bytes, bytearray)):
        return cbor_bstr(bytes(v))
    if isinstance(v, str):
        return cbor_tstr(v)
    if isinstance(v, list):
        body = b"".join(cbor(x) for x in v)
        return head(4, len(v)) + body
    if isinstance(v, dict):
        items = sorted(v.items(), key=lambda kv: cbor(kv[0]))
        body = b"".join(cbor(k) + cbor(val) for k, val in items)
        return head(5, len(items)) + body
    raise TypeError(type(v))


KEM_ID, KDF_ID, AEAD_ID = 0x0020, 0x0001, 0x0003
SUITE_KEM = b"KEM" + i2osp(KEM_ID, 2)
SUITE_HPKE = b"HPKE" + i2osp(KEM_ID, 2) + i2osp(KDF_ID, 2) + i2osp(AEAD_ID, 2)


def labeled_extract(suite_id: bytes, salt: bytes, label: bytes, ikm: bytes) -> bytes:
    return hkdf_extract(salt, b"HPKE-v1" + suite_id + label + ikm)


def labeled_expand(suite_id: bytes, prk: bytes, label: bytes, info: bytes, length: int) -> bytes:
    labeled_info = i2osp(length, 2) + b"HPKE-v1" + suite_id + label + info
    return hkdf_expand(prk, labeled_info, length)


def encap(sk_e: bytes, pk_r: bytes):
    pk_e = crypto_scalarmult_base(sk_e)
    dh = crypto_scalarmult(sk_e, pk_r)
    return pk_e, labeled_expand(
        SUITE_KEM,
        labeled_extract(SUITE_KEM, b"", b"eae_prk", dh),
        b"shared_secret",
        pk_e + pk_r,
        32,
    )


def key_schedule(shared_secret: bytes, info: bytes):
    psk_id_hash = labeled_extract(SUITE_HPKE, b"", b"psk_id_hash", b"")
    info_hash = labeled_extract(SUITE_HPKE, b"", b"info_hash", info)
    context = bytes([0]) + psk_id_hash + info_hash
    secret = labeled_extract(SUITE_HPKE, shared_secret, b"secret", b"")
    key = labeled_expand(SUITE_HPKE, secret, b"key", context, 32)
    nonce = labeled_expand(SUITE_HPKE, secret, b"base_nonce", context, 12)
    return key, nonce


def seal_base(sk_e: bytes, pk_r: bytes, info: bytes, aad: bytes, pt: bytes):
    enc, shared = encap(sk_e, pk_r)
    key, nonce = key_schedule(shared, info)
    ct = crypto_aead_chacha20poly1305_ietf_encrypt(pt, aad, nonce, key)
    return enc, ct, shared, key, nonce


def check_rfc_vector():
    sk_e = unhex("f4ec9b33b792c372c1d2c2063507b684ef925b8c75a42dbcbf57d63ccd381600")
    pk_r = unhex("4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a")
    info = unhex("4f6465206f6e2061204772656369616e2055726e")
    pt = unhex("4265617574792069732074727574682c20747275746820626561757479")
    aad = unhex("436f756e742d30")
    enc, ct, shared, key, nonce = seal_base(sk_e, pk_r, info, aad, pt)
    expect = {
        "enc": "1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a",
        "shared": "0bbe78490412b4bbea4812666f7916932b828bba79942424abb65244930d69a7",
        "key": "ad2744de8e17f4ebba575b3f5f5a8fa1f69c2a07f6e7500bc60ca6e3e3ec1c91",
        "nonce": "5c4d98150661b848853b547f",
        "ct": "1c5250d8034ec2b784ba2cfd69dbdb8af406cfe3ff938e131f0def8c8b60b4db21993c62ce81883d2dd1b51a28",
    }
    got = {"enc": enc.hex(), "shared": shared.hex(), "key": key.hex(), "nonce": nonce.hex(), "ct": ct.hex()}
    if got != expect:
        raise SystemExit("HPKE RFC vector mismatch:\n" + json.dumps(got, indent=2))


def write_vec(area: str, name: str, doc: dict):
    path = os.path.join(VEC, area)
    os.makedirs(path, exist_ok=True)
    with open(os.path.join(path, name), "w", encoding="utf-8") as handle:
        json.dump(doc, handle, indent=2)
        handle.write("\n")


def event_body(signer: bytes, prev, unix_ms, space, typ: str, payload: bytes) -> bytes:
    return cbor({1: 1, 2: signer, 3: prev, 4: unix_ms, 5: space, 6: typ, 7: payload})


def sign_wire(seed: bytes, body: bytes):
    key = SigningKey(seed)
    sig = key.sign(body).signature
    wire = head(4, 2) + body + cbor_bstr(sig)
    return key.verify_key.encode(), sig, wire, blake3(body).digest()


def main():
    check_rfc_vector()
    identity_root = bytes(range(32))
    identity_seed = hkdf(identity_root, b"ambient-era/v1", b"identity-sign", 32)
    seal_secret = hkdf(identity_root, b"ambient-era/v1", b"identity-seal", 32)
    recovery_mailbox = hkdf(identity_root, b"ambient-era/v1", b"recovery-mailbox", 32)
    seal_public = crypto_scalarmult_base(seal_secret)
    payload = cbor({1: "ambient-era"})
    signer = SigningKey(identity_seed).verify_key.encode()
    body = event_body(signer, None, None, None, "id.genesis", payload)
    public, signature, wire, event_id = sign_wire(identity_seed, body)

    write_vec("identity", "0001-genesis.json", {
        "description": "An identity genesis event derived from a fixed root seed.",
        "spec": "docs/spec/02-identity-log.md#derivation",
        "generator": "conformance/tools/generate.py",
        "input": {"root_seed": hx(identity_root)},
        "expect": {"ok": {
            "identity_seed": hx(identity_seed),
            "identity_public_key": hx(public),
            "identity_seal_secret": hx(seal_secret),
            "identity_seal_public": hx(seal_public),
            "recovery_mailbox": hx(recovery_mailbox),
            "event_id": hx(event_id),
            "body": hx(body),
            "signature": hx(signature),
            "wire": hx(wire),
        }},
    })
    write_vec("events", "0001-genesis-wire.json", {
        "description": "The identity genesis wire verifies and hashes to its event id.",
        "spec": "docs/spec/01-events.md#signatures",
        "generator": "conformance/tools/generate.py",
        "input": {"bytes": hx(wire)},
        "expect": {"ok": {"id": hx(event_id), "type": "id.genesis", "signer": hx(public)}},
    })

    bad = bytearray(wire)
    bad[-1] ^= 0x01
    write_vec("events", "0002-bad-signature.json", {
        "description": "The genesis wire with the last signature byte flipped.",
        "spec": "docs/spec/01-events.md#signatures",
        "generator": "conformance/tools/generate.py",
        "input": {"bytes": hx(bytes(bad))},
        "expect": {"error": "bad_signature"},
    })

    # Version is the integer 1 immediately after map key 1. Expand it to a non-minimal form.
    marker = bytes([0x01, 0x01])
    at = body.find(marker)
    nonminimal = body[: at + 1] + bytes([0x18, 0x01]) + body[at + 2 :]
    write_vec("events", "0003-nonminimal-int.json", {
        "description": "Version 1 encoded as a non-minimal integer.",
        "spec": "docs/spec/01-events.md#encoding",
        "generator": "conformance/tools/generate.py",
        "input": {"bytes": hx(nonminimal)},
        "expect": {"error": "noncanonical"},
    })

    unsorted = head(5, 2) + encode_uint(2) + cbor_bstr(public) + encode_uint(1) + encode_uint(1)
    write_vec("events", "0004-unsorted-keys.json", {
        "description": "A map with keys out of canonical byte order.",
        "spec": "docs/spec/01-events.md#encoding",
        "generator": "conformance/tools/generate.py",
        "input": {"bytes": hx(unsorted)},
        "expect": {"error": "noncanonical"},
    })
    write_vec("events", "0005-indefinite-bstr.json", {
        "description": "An indefinite-length byte string.",
        "spec": "docs/spec/01-events.md#encoding",
        "generator": "conformance/tools/generate.py",
        "input": {"bytes": "5f4100ff"},
        "expect": {"error": "noncanonical"},
    })

    sk_e = bytes([(i * 7 + 3) % 256 for i in range(32)])
    sk_r = bytes([(i * 13 + 9) % 256 for i in range(32)])
    pk_r = crypto_scalarmult_base(sk_r)
    mailbox = bytes([i + 1 for i in range(32)])
    info = b"ambient-era/hpke/v1"
    aad = b"ambient-era/aad/v1" + mailbox + pk_r
    plaintext = b"hello"
    enc, ct, _shared, _key, _nonce = seal_base(sk_e, pk_r, info, aad, plaintext)
    hint = blake3(b"ambient-era/hint/v1" + mailbox + pk_r).digest()[:8]
    blob = cbor({1: 1, 2: [{1: hint, 2: enc, 3: ct}]})
    write_vec("spaces", "0001-hpke-blob.json", {
        "description": "The plaintext hello sealed to one device key with a fixed ephemeral secret.",
        "spec": "docs/spec/03-spaces.md#sealing",
        "generator": "conformance/tools/generate.py",
        "input": {
            "ephemeral_secret": hx(sk_e),
            "recipient_secret": hx(sk_r),
            "mailbox": hx(mailbox),
            "plaintext": hx(plaintext),
        },
        "expect": {"ok": {
            "recipient_public": hx(pk_r),
            "hint": hx(hint),
            "enc": hx(enc),
            "ciphertext": hx(ct),
            "blob": hx(blob),
        }},
    })
    print("ok", hx(event_id))


if __name__ == "__main__":
    main()
