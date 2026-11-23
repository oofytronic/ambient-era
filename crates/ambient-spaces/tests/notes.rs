//! Concurrent note heads, and a revocation that drops a later event.

use std::collections::BTreeMap;

use ambient_events::{EventId, encode_unsigned, sign_wire, signing_public, verify};
use ambient_identity::is_retained;
use ambient_spaces::{
    GenesisMember, MAX_MEMBERS, Revocation, Skip, conversation_genesis_payload, member_add_payload,
    note_put_payload, notes_genesis_payload, project_conversation, project_notes,
    project_notes_for_root, project_notes_verified,
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

#[test]
fn a_verified_projection_matches_the_wire_projection() -> Result<(), String> {
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
    let wires: Vec<&[u8]> = vec![&genesis, &left_wire, &right_wire];
    let revoked = [Revocation {
        signing_key: signing_public(&other),
        up_to: None,
    }];
    for revoked in [&[][..], &revoked[..]] {
        let by_wire =
            project_notes_for_root(&wires, revoked, None).map_err(|err| err.to_string())?;
        let verified: Vec<_> = wires
            .iter()
            .map(|wire| verify(wire).map_err(|err| err.to_string()))
            .collect::<Result<_, _>>()?;
        let refs: Vec<_> = verified.iter().collect();
        let by_event =
            project_notes_verified(&refs, revoked, None).map_err(|err| err.to_string())?;
        if by_wire != by_event {
            return Err("the two projections differ".to_owned());
        }
    }
    Ok(())
}

/// A new device's first note and the identity-signed enrollment that makes it a member are both
/// ready in the same round of the merge. The order they are applied in must not depend on the
/// random event ids, or the note is dropped as `not_member` about half the time.
#[test]
fn a_new_device_note_survives_its_enrollment_in_any_id_order() -> Result<(), String> {
    let owner = seed(1);
    let root = seed(7);
    let newcomer = seed(2);
    let genesis_payload = notes_genesis_payload(&[GenesisMember {
        signing_key: signing_public(&owner),
        encryption_key: seed(3),
        role: 1,
        mailbox: seed(4),
    }])
    .map_err(|err| err.to_string())?;
    let genesis = sign(&owner, None, None, "space.genesis", &genesis_payload)?;
    let space_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let enroll_payload = member_add_payload(&GenesisMember {
        signing_key: signing_public(&newcomer),
        encryption_key: seed(5),
        role: 1,
        mailbox: seed(6),
    })
    .map_err(|err| err.to_string())?;
    let enroll = sign(
        &root,
        None,
        Some(space_id),
        "space.member-add",
        &enroll_payload,
    )?;
    let enroll_id = verify(&enroll).map_err(|err| err.to_string())?.id;

    let (mut note_first, mut enroll_first) = (0_u32, 0_u32);
    for round in 0..64_u8 {
        let note_id = seed(round.wrapping_add(10));
        let payload = note_put_payload(&note_id, "from the new device", "text", &[])
            .map_err(|err| err.to_string())?;
        let wire = sign(&newcomer, None, Some(space_id), "note.put", &payload)?;
        let id = verify(&wire).map_err(|err| err.to_string())?.id;
        if id < enroll_id {
            note_first += 1;
        } else {
            enroll_first += 1;
        }
        let view = project_notes_for_root(
            &[&genesis, &enroll, &wire],
            &[],
            Some(signing_public(&root)),
        )
        .map_err(|err| err.to_string())?;
        if !view.notes.contains_key(&note_id) {
            return Err(format!(
                "the note was dropped (note sorts first: {}, skipped: {:?})",
                id < enroll_id,
                view.skipped
            ));
        }
    }
    if note_first == 0 || enroll_first == 0 {
        return Err(format!(
            "the test never saw both orders ({note_first} / {enroll_first})"
        ));
    }
    Ok(())
}

/// The owner's own device signs the enrollment, so it only becomes ready after the owner's
/// earlier events. The new device's first note is ready at once. It has to wait for the
/// enrollment instead of being dropped as `not_member` before the enrollment is applied.
#[test]
fn a_first_note_waits_for_an_enrollment_that_is_a_round_behind() -> Result<(), String> {
    let owner = seed(1);
    let newcomer = seed(2);
    let genesis_payload = notes_genesis_payload(&[GenesisMember {
        signing_key: signing_public(&owner),
        encryption_key: seed(3),
        role: 1,
        mailbox: seed(4),
    }])
    .map_err(|err| err.to_string())?;
    let genesis = sign(&owner, None, None, "space.genesis", &genesis_payload)?;
    let space_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let first =
        note_put_payload(&seed(9), "owner note", "text", &[]).map_err(|err| err.to_string())?;
    let first_wire = sign(&owner, Some(space_id), Some(space_id), "note.put", &first)?;
    let first_id = verify(&first_wire).map_err(|err| err.to_string())?.id;
    let enroll_payload = member_add_payload(&GenesisMember {
        signing_key: signing_public(&newcomer),
        encryption_key: seed(5),
        role: 2,
        mailbox: seed(6),
    })
    .map_err(|err| err.to_string())?;
    let enroll = sign(
        &owner,
        Some(first_id),
        Some(space_id),
        "space.member-add",
        &enroll_payload,
    )?;
    let theirs = note_put_payload(&seed(8), "from the new device", "text", &[])
        .map_err(|err| err.to_string())?;
    let theirs_wire = sign(&newcomer, None, Some(space_id), "note.put", &theirs)?;
    let theirs_id = verify(&theirs_wire).map_err(|err| err.to_string())?.id;
    let later = note_put_payload(&seed(7), "its second note", "text", &[])
        .map_err(|err| err.to_string())?;
    let later_wire = sign(
        &newcomer,
        Some(theirs_id),
        Some(space_id),
        "note.put",
        &later,
    )?;

    let wires: [&[u8]; 5] = [&genesis, &first_wire, &enroll, &theirs_wire, &later_wire];
    // Every arrival order gives the same notes.
    for rotate in 0..wires.len() {
        let mut order = wires.to_vec();
        order.rotate_left(rotate);
        let view = project_notes(&order, &[]).map_err(|err| err.to_string())?;
        for want in [seed(9), seed(8), seed(7)] {
            if !view.notes.contains_key(&want) {
                return Err(format!("a note was dropped: {:?}", view.skipped));
            }
        }
        if !view.skipped.is_empty() {
            return Err(format!("nothing should be skipped: {:?}", view.skipped));
        }
    }
    // A signer nothing ever enrolls is still refused.
    let stranger = seed(11);
    let stray =
        note_put_payload(&seed(6), "uninvited", "text", &[]).map_err(|err| err.to_string())?;
    let stray_wire = sign(&stranger, None, Some(space_id), "note.put", &stray)?;
    let view = project_notes(&[&genesis, &first_wire, &enroll, &stray_wire], &[])
        .map_err(|err| err.to_string())?;
    if view.notes.contains_key(&seed(6))
        || !view.skipped.iter().any(|(_, why)| *why == Skip::NotMember)
    {
        return Err("a stranger's note was applied".to_owned());
    }
    Ok(())
}

#[test]
fn a_seventeenth_member_is_not_added() -> Result<(), String> {
    let mut members = Vec::new();
    for index in 0..MAX_MEMBERS {
        let fill = u8::try_from(index).map_err(|err| err.to_string())?;
        members.push(GenesisMember {
            signing_key: signing_public(&seed(fill)),
            encryption_key: seed(fill.wrapping_add(40)),
            role: if index == 0 { 1 } else { 2 },
            mailbox: seed(fill.wrapping_add(80)),
        });
    }
    let payload = conversation_genesis_payload(&members).map_err(|err| err.to_string())?;
    let owner = seed(0);
    let genesis = sign(&owner, None, None, "space.genesis", &payload)?;
    let space_id = verify(&genesis).map_err(|err| err.to_string())?.id;
    let extra = GenesisMember {
        signing_key: signing_public(&seed(30)),
        encryption_key: seed(31),
        role: 2,
        mailbox: seed(32),
    };
    let add = member_add_payload(&extra).map_err(|err| err.to_string())?;
    let add_wire = sign(
        &owner,
        Some(space_id),
        Some(space_id),
        "space.member-add",
        &add,
    )?;
    let view = project_conversation(&[&genesis, &add_wire], &[]).map_err(|err| err.to_string())?;
    if view.members.len() != MAX_MEMBERS {
        return Err(format!("cap was not kept: {}", view.members.len()));
    }
    if !view.skipped.iter().any(|(_, why)| *why == Skip::Full) {
        return Err(format!(
            "the extra member was not refused: {:?}",
            view.skipped
        ));
    }
    let mut too_many = members;
    too_many.push(extra);
    if conversation_genesis_payload(&too_many).is_ok() {
        return Err("genesis accepted 17 members".to_owned());
    }
    Ok(())
}
