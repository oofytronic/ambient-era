//! Spike 6: a minimal MLS group lifecycle on OpenMLS, used to measure sizes, timing, and
//! behavior relevant to `ambient-spaces` and ADR-0001 (add, message, remove, removed member).

use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const CS: Ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

pub struct Member {
    pub provider: OpenMlsRustCrypto,
    pub signer: SignatureKeyPair,
    pub cred: CredentialWithKey,
}

pub fn member(name: &str) -> Result<Member> {
    let provider = OpenMlsRustCrypto::default();
    let credential = BasicCredential::new(name.as_bytes().to_vec());
    let signer = SignatureKeyPair::new(CS.signature_algorithm())?;
    signer.store(provider.storage())?;
    let cred = CredentialWithKey {
        credential: credential.into(),
        signature_key: signer.public().into(),
    };
    Ok(Member {
        provider,
        signer,
        cred,
    })
}

pub fn key_package(m: &Member) -> Result<KeyPackage> {
    let bundle = KeyPackage::builder().build(CS, &m.provider, &m.signer, m.cred.clone())?;
    Ok(bundle.key_package().clone())
}

pub fn new_group(m: &Member) -> Result<MlsGroup> {
    let config = MlsGroupCreateConfig::builder()
        .ciphersuite(CS)
        .use_ratchet_tree_extension(true)
        .build();
    Ok(MlsGroup::new(
        &m.provider,
        &m.signer,
        &config,
        m.cred.clone(),
    )?)
}

pub fn join(m: &Member, welcome_bytes: &[u8]) -> Result<MlsGroup> {
    let msg = MlsMessageIn::tls_deserialize_exact(welcome_bytes)?;
    let MlsMessageBodyIn::Welcome(welcome) = msg.extract() else {
        return Err("not a welcome".into());
    };
    let staged = StagedWelcome::new_from_welcome(
        &m.provider,
        &MlsGroupJoinConfig::default(),
        welcome,
        None,
    )?;
    Ok(staged.into_group(&m.provider)?)
}

/// Deliver serialized bytes to a member's group. Returns application plaintext, if any.
pub fn deliver(m: &Member, group: &mut MlsGroup, bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    let msg = MlsMessageIn::tls_deserialize_exact(bytes)?;
    let processed = group.process_message(&m.provider, msg.try_into_protocol_message()?)?;
    match processed.into_content() {
        ProcessedMessageContent::ApplicationMessage(app) => Ok(Some(app.into_bytes())),
        ProcessedMessageContent::StagedCommitMessage(staged) => {
            group.merge_staged_commit(&m.provider, *staged)?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

pub fn send(m: &Member, group: &mut MlsGroup, plaintext: &[u8]) -> Result<Vec<u8>> {
    Ok(group
        .create_message(&m.provider, &m.signer, plaintext)?
        .tls_serialize_detached()?)
}

/// Sizes in bytes.
#[derive(Debug, Default)]
pub struct Small {
    pub key_package: usize,
    pub welcome_2: usize,
    pub commit_add_third: usize,
    pub welcome_3: usize,
    pub app_overhead_32b: usize,
    pub removal_commit: usize,
    pub removed_member_cannot_read_later_message: bool,
    pub removed_member_sees_removal_commit: bool,
    pub others_still_read: bool,
}

/// Alice creates a group, adds Bob, they talk, Alice adds Carol, then removes Bob.
pub fn small_scenario() -> Result<Small> {
    let (alice, bob, carol) = (member("alice")?, member("bob")?, member("carol")?);
    let mut out = Small::default();

    let mut ga = new_group(&alice)?;
    let kp_bob = key_package(&bob)?;
    out.key_package = kp_bob.tls_serialize_detached()?.len();

    let (_commit, welcome, _gi) = ga.add_members(&alice.provider, &alice.signer, &[kp_bob])?;
    ga.merge_pending_commit(&alice.provider)?;
    let welcome_bytes = welcome.tls_serialize_detached()?;
    out.welcome_2 = welcome_bytes.len();
    let mut gb = join(&bob, &welcome_bytes)?;

    let ct = send(&alice, &mut ga, b"hello bob")?;
    assert_eq!(
        deliver(&bob, &mut gb, &ct)?.as_deref(),
        Some(&b"hello bob"[..])
    );
    let ct = send(&bob, &mut gb, b"hello alice")?;
    assert_eq!(
        deliver(&alice, &mut ga, &ct)?.as_deref(),
        Some(&b"hello alice"[..])
    );

    let kp_carol = key_package(&carol)?;
    let (commit, welcome, _gi) = ga.add_members(&alice.provider, &alice.signer, &[kp_carol])?;
    ga.merge_pending_commit(&alice.provider)?;
    let commit_bytes = commit.tls_serialize_detached()?;
    out.commit_add_third = commit_bytes.len();
    deliver(&bob, &mut gb, &commit_bytes)?;
    let welcome_bytes = welcome.tls_serialize_detached()?;
    out.welcome_3 = welcome_bytes.len();
    let mut gc = join(&carol, &welcome_bytes)?;

    let plain = [7u8; 32];
    let ct = send(&alice, &mut ga, &plain)?;
    out.app_overhead_32b = ct.len() - plain.len();
    deliver(&bob, &mut gb, &ct)?;
    deliver(&carol, &mut gc, &ct)?;

    // Remove Bob. ADR-0001: a removed member must not read what comes after.
    let bob_leaf = gb.own_leaf_index();
    let (commit, _welcome, _gi) = ga.remove_members(&alice.provider, &alice.signer, &[bob_leaf])?;
    ga.merge_pending_commit(&alice.provider)?;
    let removal = commit.tls_serialize_detached()?;
    out.removal_commit = removal.len();
    deliver(&carol, &mut gc, &removal)?;

    let after = send(&alice, &mut ga, b"after bob left")?;
    out.others_still_read =
        deliver(&carol, &mut gc, &after)?.as_deref() == Some(&b"after bob left"[..]);
    // Bob never saw the removal commit (offline): he must fail to read the new epoch.
    out.removed_member_cannot_read_later_message = deliver(&bob, &mut gb, &after).is_err();
    // Bob processes the removal commit: his group must become inactive.
    deliver(&bob, &mut gb, &removal)?;
    out.removed_member_sees_removal_commit = !gb.is_active();
    Ok(out)
}

#[derive(Debug)]
pub struct Scale {
    pub members: usize,
    pub commit_add_all: usize,
    pub welcome: usize,
    pub update_commit: usize,
}

/// Alice adds `n - 1` members in one commit; then one member self-updates (typical key rotation).
pub fn scale_scenario(n: usize) -> Result<Scale> {
    let alice = member("alice")?;
    let others: Vec<Member> = (1..n)
        .map(|i| member(&format!("m{i}")))
        .collect::<Result<_>>()?;
    let kps: Vec<KeyPackage> = others.iter().map(key_package).collect::<Result<_>>()?;
    let mut ga = new_group(&alice)?;
    let (commit, welcome, _gi) = ga.add_members(&alice.provider, &alice.signer, &kps)?;
    ga.merge_pending_commit(&alice.provider)?;
    let commit_add_all = commit.tls_serialize_detached()?.len();
    let welcome_bytes = welcome.tls_serialize_detached()?;
    let _joined = join(&others[0], &welcome_bytes)?;
    let (update, _welcome, _gi) = ga
        .self_update(
            &alice.provider,
            &alice.signer,
            LeafNodeParameters::default(),
        )?
        .into_messages();
    ga.merge_pending_commit(&alice.provider)?;
    let update_commit = update.tls_serialize_detached()?.len();
    Ok(Scale {
        members: n,
        commit_add_all,
        welcome: welcome_bytes.len(),
        update_commit,
    })
}

/// Entry point for the wasm build: runs the small scenario, returns 1 on success.
#[unsafe(no_mangle)]
pub extern "C" fn spike_run() -> u32 {
    match small_scenario() {
        Ok(s) if s.removed_member_cannot_read_later_message && s.others_still_read => 1,
        _ => 0,
    }
}
