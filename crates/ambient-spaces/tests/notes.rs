//! Concurrent note heads, and a revocation that drops a later event.

use std::collections::BTreeMap;

use ambient_events::{EventId, encode_unsigned, sign_wire, signing_public, verify};
use ambient_identity::is_retained;
use ambient_spaces::{
    GenesisMember, Revocation, Skip, note_put_payload, notes_genesis_payload, project_notes,
};

fn seed(fill: u8) -> [u8; 32] {
    [fill; 32]
}

fn sign(
    seed: &[u8; 32],
    prev: Option<EventId>,
    space: Option<EventId>,
    event_type: &str,
    payload: &[u8],
) -> Result<Vec<u8>, String> {
    let unsigned = encode_unsigned(
        &signing_public(seed),
        prev.map(|id| id.0),
        None,
        space.map(|id| id.0),
        event_type,
        payload,
    )
    .map_err(|err| err.to_string())?;
    sign_wire(&unsigned, seed).map_err(|err| err.to_string())
}

#[test]
fn concurrent_notes_stay_in_conflict() -> Result<(), String> {
    let owner = seed(1);
    let other = seed(2);
    let payload = notes_genesis_payload(&[
        GenesisMember {
            signing_key: signing_public(&owner),
            encryption_key: seed(3),
            role: 1,
            mailbox: seed(4),
        },
        GenesisMember {
            signing_key: signing_public(&other),
            encryption_key: seed(5),
            role: 2,
            mailbox: seed(6),
        },
    ])
    .map_err(|err| err.to_string())?;
    let genesis = sign(&owner, None, None, "space.genesis", &payload)?;
    let space_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let note = seed(9);
    let left = note_put_payload(&note, "left", "alpha", &[]).map_err(|err| err.to_string())?;
    let right = note_put_payload(&note, "right", "beta", &[]).map_err(|err| err.to_string())?;
    let left_wire = sign(&owner, Some(space_id), Some(space_id), "note.put", &left)?;
    let right_wire = sign(&other, None, Some(space_id), "note.put", &right)?;
    let left_id = verify(&left_wire).map_err(|err| err.to_string())?.id;
    let right_id = verify(&right_wire).map_err(|err| err.to_string())?.id;
    let view =
        project_notes(&[&genesis, &left_wire, &right_wire], &[]).map_err(|err| err.to_string())?;
    let heads = view.notes.get(&note).ok_or("missing note")?;
    if heads.len() != 2 {
        return Err(format!("expected a conflict, got {} heads", heads.len()));
    }
    let resolved = note_put_payload(&note, "merged", "both", &[left_id, right_id])
        .map_err(|err| err.to_string())?;
    let resolved_wire = sign(&owner, Some(left_id), Some(space_id), "note.put", &resolved)?;
    let view = project_notes(&[&genesis, &left_wire, &right_wire, &resolved_wire], &[])
        .map_err(|err| err.to_string())?;
    let heads = view.notes.get(&note).ok_or("missing note")?;
    if heads.len() != 1 || heads.first().map(|head| head.body.as_str()) != Some("both") {
        return Err("merge did not become the only head".to_owned());
    }
    Ok(())
}

#[test]
fn revoked_event_is_not_applied() -> Result<(), String> {
    let owner = seed(1);
    let payload = notes_genesis_payload(&[GenesisMember {
        signing_key: signing_public(&owner),
        encryption_key: seed(3),
        role: 1,
        mailbox: seed(4),
    }])
    .map_err(|err| err.to_string())?;
    let genesis = sign(&owner, None, None, "space.genesis", &payload)?;
    let space_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let note = seed(8);
    let body = note_put_payload(&note, "secret", "nope", &[]).map_err(|err| err.to_string())?;
    let wire = sign(&owner, Some(space_id), Some(space_id), "note.put", &body)?;
    let note_id = verify(&wire).map_err(|err| err.to_string())?.id;
    let view = project_notes(
        &[&genesis, &wire],
        &[Revocation {
            signing_key: signing_public(&owner),
            up_to: Some(space_id),
        }],
    )
    .map_err(|err| err.to_string())?;
    if view.notes.contains_key(&note) {
        return Err("revoked note was applied".to_owned());
    }
    if !view.skipped.iter().any(|(_, skip)| *skip == Skip::Revoked) {
        return Err("missing revoked skip".to_owned());
    }
    let mut prevs = BTreeMap::new();
    prevs.insert(space_id, None);
    prevs.insert(note_id, Some(space_id));
    if !is_retained(Some(space_id), space_id, &prevs)
        || is_retained(Some(space_id), note_id, &prevs)
    {
        return Err("retention walk disagreed with the projection".to_owned());
    }
    Ok(())
}
