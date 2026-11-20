# ADR-0006: A note from a signer who is not yet a member waits for an enrollment

**Status:** Proposed. Implemented on the `test` branch in commit `398b78c`. `ambient-spaces` needs owner review before this is Accepted. The rule is already written into [spec 03](../spec/03-spaces.md) under Merge.

## Context

A Space is a membership list and the merge of each member's signed log. The merge applies events in rounds. In each round it takes every event that is ready, which means its predecessor from the same signer has been applied, and applies them in the order of their event ids.

A device's first event has no predecessor, so it is ready at once. The `space.member-add` that enrolls the device is signed by someone else, either the root or the owner's device, and it may be ready in the same round or one round later. Event ids are hashes, so which of the two has the smaller id is random.

When the note came first, its signer was not a member yet. The merge skipped it as `not_member` and moved that signer's chain past it. The note was gone from the projection for good, on every device that merged those events. A device that was just paired or restored lost its first note about four times in ten. This is what made `restored_folder_writes_back` fail intermittently.

Applying enrollments before notes within a round was tried first. It fixed the case where the enrollment was ready in the same round, and it did not fix the case where the owner's device signed the enrollment and it only became ready after that device's earlier notes.

## Decision

A `note.put` or `msg.put` from a signer who is not a writer yet waits. It stays in the merge and is looked at again after every round. Only when a whole round applies nothing is it clear that no event left can enroll the signer. Then the event is refused as `not_member`, as before, and the signer's chain moves past it.

Membership events keep the order of their ids among the other events. Removals are unchanged. A signer that nothing ever enrolls is still refused.

This changes no byte on the wire, no signature rule, and no key. It changes which events a projection applies, so it is written into spec 03.

## Consequences

- The projection of a set of events no longer depends on random ids or on the order they arrived in, for this case. A test covers both shapes and every rotation of the arrival order.
- A note from a device that is enrolled later in the same batch is kept. Nothing that was refused before is now accepted unless an enrollment for the signer is in the merge.
- The merge can take more rounds. Each round that applies something makes progress, and a round that applies nothing ends the loop, so it terminates.
- What is not fixed: a removal and a note from the removed member, with neither referring to the other, still depend on id order. Revoking a device already has `up_to` to say what is kept. Removing a member from a Space has no such marker.
- There is no conformance vector, because there is no vector format for a projection yet. The tests in `crates/ambient-spaces/tests/notes.rs` stand in for it. A second implementation would need this written as a vector first.

## Alternatives considered

- **Enrollments first within a round.** Rejected. It is not enough, as described above.
- **A causal order across signers.** Would need a new field that links an event to the enrollment it depends on. That is a wire change and a new spec version, for a problem a merge rule solves.
- **Accept events from non-members.** Rejected. It would break the rule that read-only is enforced by every client ignoring events from members without the role.
