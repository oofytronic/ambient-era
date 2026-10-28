//! Device add, revoke retention, forks, and pairing offers.

use std::collections::BTreeMap;

use ambient_events::{EventId, encode_unsigned, sign_wire, verify};
use ambient_identity::derive;
use ambient_identity::{
    DeviceState, IdentityError, decode_pair_offer_text, device_add_payload, device_from_seeds,
    encode_pair_offer, inbox_payload, is_retained, pair_offer_text, project_log, revoke_payload,
};

fn root() -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::try_from(index).unwrap_or(0);
    }
    bytes
}

fn seeds(fill: u8) -> [u8; 32] {
    [fill; 32]
}

#[test]
fn add_inbox_and_revoke() -> Result<(), String> {
    let identity = derive(&root()).map_err(|err| err.to_string())?;
    let device = device_from_seeds(&seeds(1), &seeds(2), &seeds(3), "laptop")
        .map_err(|err| err.to_string())?;
    let genesis = identity.genesis_wire().map_err(|err| err.to_string())?;
    let genesis_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let add = device_add_payload(
        &device.signing_key,
        &device.encryption_key,
        "laptop",
        &device.mailbox,
    )
    .map_err(|err| err.to_string())?;
    let add_wire = identity
        .sign_log(Some(genesis_id), "id.device-add", &add)
        .map_err(|err| err.to_string())?;
    let add_id = verify(&add_wire).map_err(|err| err.to_string())?.id;
    let inbox = inbox_payload(&device.signing_key, &seeds(9)).map_err(|err| err.to_string())?;
    let inbox_wire = identity
        .sign_log(Some(add_id), "id.inbox", &inbox)
        .map_err(|err| err.to_string())?;
    let inbox_id = verify(&inbox_wire).map_err(|err| err.to_string())?.id;
    let revoke =
        revoke_payload(&device.signing_key, Some(add_id)).map_err(|err| err.to_string())?;
    let revoke_wire = identity
        .sign_log(Some(inbox_id), "id.device-revoke", &revoke)
        .map_err(|err| err.to_string())?;
    let projected = project_log(
        &identity.signing_key,
        &[&genesis, &add_wire, &inbox_wire, &revoke_wire],
    )
    .map_err(|err| err.to_string())?;
    match projected.devices.get(&device.signing_key) {
        Some(DeviceState::Revoked { mailbox, up_to, .. }) => {
            if mailbox != &seeds(9) || *up_to != Some(add_id) {
                return Err("revoke projection mismatch".to_owned());
            }
        }
        _ => return Err("device was not revoked".to_owned()),
    }
    let mut prevs = BTreeMap::new();
    prevs.insert(add_id, Some(genesis_id));
    prevs.insert(inbox_id, Some(add_id));
    if !is_retained(Some(add_id), genesis_id, &prevs) {
        return Err("ancestor should be retained".to_owned());
    }
    if is_retained(Some(add_id), inbox_id, &prevs) {
        return Err("event after up_to should not be retained".to_owned());
    }
    if is_retained(None, add_id, &prevs) {
        return Err("null up_to retains nothing".to_owned());
    }
    Ok(())
}

#[test]
fn second_genesis_is_rejected() -> Result<(), String> {
    let identity = derive(&root()).map_err(|err| err.to_string())?;
    let first = identity.genesis_wire().map_err(|err| err.to_string())?;
    let first_id = verify(&first).map_err(|err| err.to_string())?.id;
    let payload = encode_unsigned(
        &identity.signing_key,
        Some(first_id.0),
        None,
        None,
        "id.genesis",
        b"\xa1\x01\x6bambient-era",
    )
    .map_err(|err| err.to_string())?;
    let second = sign_wire(&payload, identity.signing_seed()).map_err(|err| err.to_string())?;
    let err = project_log(&identity.signing_key, &[&first, &second])
        .err()
        .ok_or("second genesis was accepted")?;
    if err != IdentityError::BadPayload {
        return Err(format!("got {err}"));
    }
    Ok(())
}

#[test]
fn smaller_fork_wins() -> Result<(), String> {
    let identity = derive(&root()).map_err(|err| err.to_string())?;
    let genesis = identity.genesis_wire().map_err(|err| err.to_string())?;
    let genesis_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let one =
        device_from_seeds(&seeds(4), &seeds(5), &seeds(6), "one").map_err(|err| err.to_string())?;
    let two =
        device_from_seeds(&seeds(7), &seeds(8), &seeds(9), "two").map_err(|err| err.to_string())?;
    let add_one = identity
        .sign_log(
            Some(genesis_id),
            "id.device-add",
            &device_add_payload(&one.signing_key, &one.encryption_key, "one", &one.mailbox)
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
    let add_two = identity
        .sign_log(
            Some(genesis_id),
            "id.device-add",
            &device_add_payload(&two.signing_key, &two.encryption_key, "two", &two.mailbox)
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;
    let id_one = verify(&add_one).map_err(|err| err.to_string())?.id;
    let id_two = verify(&add_two).map_err(|err| err.to_string())?.id;
    let projected = project_log(&identity.signing_key, &[&genesis, &add_one, &add_two])
        .map_err(|err| err.to_string())?;
    let (winner, loser) = if id_one.0 < id_two.0 {
        (&one, id_two)
    } else {
        (&two, id_one)
    };
    if !projected.devices.contains_key(&winner.signing_key) {
        return Err("smaller fork was not applied".to_owned());
    }
    if projected.devices.len() != 1 || projected.forks != vec![loser] {
        return Err("fork set mismatch".to_owned());
    }
    Ok(())
}

#[test]
fn pair_offer_roundtrip() -> Result<(), String> {
    let signing = seeds(1);
    let encryption = seeds(2);
    let mailbox = seeds(3);
    let bytes =
        encode_pair_offer(&signing, &encryption, &mailbox).map_err(|err| err.to_string())?;
    let text = pair_offer_text(&bytes);
    if text.contains('+') || text.contains('/') || text.contains('=') {
        return Err("offer text is not unpadded base64url".to_owned());
    }
    let offer = decode_pair_offer_text(&text).map_err(|err| err.to_string())?;
    if offer.signing_key != signing
        || offer.encryption_key != encryption
        || offer.mailbox != mailbox
        || offer.version != 1
    {
        return Err("offer mismatch".to_owned());
    }
    let _: EventId = EventId(signing);
    Ok(())
}
