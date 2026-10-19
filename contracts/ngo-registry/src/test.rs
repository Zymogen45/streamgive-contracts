// SPDX-License-Identifier: Apache-2.0
#![cfg(test)]

use super::*;
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{Address as _, AuthorizedFunction, Events as _, Ledger};
use soroban_sdk::{symbol_short, vec, IntoVal, Symbol, Val, Vec};

fn setup() -> (Env, NgoRegistryClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    // Leave room for the 90-day NGO bump so the TTL tests see it unclamped.
    env.ledger()
        .with_mut(|l| l.max_entry_ttl = 365 * DAY_IN_LEDGERS);

    let admin = Address::generate(&env);
    let contract_id = env.register(NgoRegistry, NgoRegistryArgs::__constructor(&admin));
    let client = NgoRegistryClient::new(&env, &contract_id);

    (env, client, admin)
}

#[test]
fn init_sets_admin() {
    let (_env, client, admin) = setup();
    assert_eq!(client.admin(), admin);
}

#[test]
fn double_init_fails() {
    let (_env, client, admin) = setup();
    let result = client.try_init(&admin);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
#[should_panic]
fn init_without_admin_auth_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(NgoRegistry, NgoRegistryArgs::__constructor(&admin));
    let client = NgoRegistryClient::new(&env, &contract_id);

    let attacker = Address::generate(&env);
    env.mock_auths(&[]);
    client.init(&attacker);
}

#[test]
fn constructor_publishes_init_event() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    // The constructor is the latest top-level invocation, so its event is the
    // only contract event in the buffer.
    let contract_id = env.register(NgoRegistry, NgoRegistryArgs::__constructor(&admin));

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                contract_id.clone(),
                (symbol_short!("init"),).into_val(&env),
                admin.into_val(&env),
            ),
        ]
    );
}

#[test]
fn register_ngo_stores_unverified_entry() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");

    client.register(&owner, &name);

    let ngo = client.get_ngo(&owner);
    assert_eq!(ngo.owner, owner);
    assert_eq!(ngo.name, name);
    assert!(!ngo.verified);
}

#[test]
fn register_whitespace_only_name_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let whitespace = String::from_str(&env, " \t\n\r");

    let result = client.try_register(&owner, &whitespace);

    assert_eq!(result, Err(Ok(Error::InvalidName)));
    assert_eq!(client.try_get_ngo(&owner), Err(Ok(Error::NotRegistered)));
}

#[test]
fn unverified_ngo_can_unregister_and_register_again() {
    let (env, client, _) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Pending NGO");

    client.register(&owner, &name);
    client.unregister(&owner);
    assert_eq!(client.try_get_ngo(&owner), Err(Ok(Error::NotRegistered)));

    client.register(&owner, &String::from_str(&env, "Updated NGO"));
    assert_eq!(
        client.get_ngo(&owner).name,
        String::from_str(&env, "Updated NGO")
    );
}

#[test]
fn double_register_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");

    client.register(&owner, &name);
    let result = client.try_register(&owner, &name);

    assert_eq!(result, Err(Ok(Error::AlreadyRegistered)));
}

#[test]
fn register_rejects_name_over_max_length() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    // MAX_NGO_NAME_LEN (200) + 1 bytes.
    let too_long = String::from_str(&env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    let result = client.try_register(&owner, &too_long);

    assert_eq!(result, Err(Ok(Error::NameTooLong)));
    assert_eq!(client.try_get_ngo(&owner), Err(Ok(Error::NotRegistered)));
}

#[test]
fn register_accepts_name_at_max_length() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    // Exactly MAX_NGO_NAME_LEN (200) bytes.
    let at_limit = String::from_str(&env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    client.register(&owner, &at_limit);

    assert_eq!(client.get_ngo(&owner).name, at_limit);
}

#[test]
fn update_name_rejects_name_over_max_length() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    let too_long = String::from_str(&env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    let result = client.try_update_name(&owner, &too_long);

    assert_eq!(result, Err(Ok(Error::NameTooLong)));
}

#[test]
fn update_name_accepts_name_at_max_length() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Initial NGO"));
    // Exactly MAX_NGO_NAME_LEN (200) bytes.
    let at_limit = String::from_str(&env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");

    client.update_name(&owner, &at_limit);
    assert_eq!(client.get_ngo(&owner).name, at_limit);
}

#[test]
fn register_and_update_name_unicode_whitespace_only_fails() {
    let (env, client, _admin) = setup();
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    client.register(&owner2, &String::from_str(&env, "Valid NGO"));

    let test_cases = [
        "\u{00a0}",                         // Non-breaking space
        "\u{3000}",                         // Ideographic space
        "\u{2003}",                         // Em space
        " \t\u{00a0}\u{3000}\u{2003}\n\r ", // Mix with ASCII whitespace
    ];

    for case in test_cases.iter() {
        let ws = String::from_str(&env, case);
        let res_reg = client.try_register(&owner1, &ws);
        assert_eq!(res_reg, Err(Ok(Error::InvalidName)));

        let res_upd = client.try_update_name(&owner2, &ws);
        assert_eq!(res_upd, Err(Ok(Error::InvalidName)));
    }
}

#[test]
fn register_and_update_name_zero_width_invisible_only_fails() {
    let (env, client, _admin) = setup();
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    client.register(&owner2, &String::from_str(&env, "Valid NGO"));

    let test_cases = [
        "\u{200b}",                                 // Zero-width space
        "\u{200c}",                                 // Zero-width non-joiner
        "\u{200d}",                                 // Zero-width joiner
        "\u{2060}",                                 // Word joiner
        "\u{feff}",                                 // BOM / Zero-width no-break space
        "\u{200b}\u{200c}\u{200d}\u{2060}\u{feff}", // All invisible
        " \u{200b} \t\u{feff}\n",                   // Mixed with whitespace
    ];

    for case in test_cases.iter() {
        let inv = String::from_str(&env, case);
        let res_reg = client.try_register(&owner1, &inv);
        assert_eq!(res_reg, Err(Ok(Error::InvalidName)));

        let res_upd = client.try_update_name(&owner2, &inv);
        assert_eq!(res_upd, Err(Ok(Error::InvalidName)));
    }
}

#[test]
fn register_and_update_name_control_characters_fail() {
    let (env, client, _admin) = setup();
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    client.register(&owner2, &String::from_str(&env, "Valid NGO"));

    let test_cases = [
        "Charity\u{0000}Name", // NUL
        "Charity\u{0007}Name", // BEL
        "Charity\u{001b}Name", // ESC
        "Charity\u{007f}Name", // DEL
        "Charity\u{0080}Name", // C1 control (U+0080)
        "Charity\u{009f}Name", // C1 control (U+009F)
    ];

    for case in test_cases.iter() {
        let ctrl = String::from_str(&env, case);
        let res_reg = client.try_register(&owner1, &ctrl);
        assert_eq!(res_reg, Err(Ok(Error::InvalidName)));

        let res_upd = client.try_update_name(&owner2, &ctrl);
        assert_eq!(res_upd, Err(Ok(Error::InvalidName)));
    }
}

#[test]
fn register_and_update_name_internal_unicode_space_succeeds() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name_ideographic = String::from_str(&env, "Red\u{3000}Cross");
    let name_nbsp = String::from_str(&env, "Red\u{00a0}Cross");

    // Register with internal ideographic space succeeds
    client.register(&owner, &name_ideographic);
    assert_eq!(client.get_ngo(&owner).name, name_ideographic);

    // Update with internal non-breaking space succeeds
    client.update_name(&owner, &name_nbsp);
    assert_eq!(client.get_ngo(&owner).name, name_nbsp);
}

#[test]
fn register_and_update_name_byte_boundaries_and_multibyte() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    // Greek capital Omega 'Ω' (U+03A9) is 2 bytes in UTF-8 (0xCE 0xA9).
    // 100 * 2 bytes = 200 bytes, exactly MAX_NGO_NAME_LEN.
    let omega_100_str = "ΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩ";
    assert_eq!(omega_100_str.len(), 200);
    let omega_100 = String::from_str(&env, omega_100_str);

    client.register(&owner, &omega_100);
    assert_eq!(client.get_ngo(&owner).name, omega_100);

    // 101 Greek capital Omegas = 202 bytes > 200 bytes, rejected with NameTooLong.
    let omega_101_str = "ΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩΩ";
    assert_eq!(omega_101_str.len(), 202);
    let omega_101 = String::from_str(&env, omega_101_str);

    let res_upd = client.try_update_name(&owner, &omega_101);
    assert_eq!(res_upd, Err(Ok(Error::NameTooLong)));

    let owner2 = Address::generate(&env);
    let res_reg = client.try_register(&owner2, &omega_101);
    assert_eq!(res_reg, Err(Ok(Error::NameTooLong)));

    // Multi-byte at boundary: 198 'a's + 1 'Ω' (2 bytes) = 200 bytes -> accepted
    let boundary_200_str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaΩ";
    assert_eq!(boundary_200_str.len(), 200);
    let boundary_200 = String::from_str(&env, boundary_200_str);
    client.update_name(&owner, &boundary_200);
    assert_eq!(client.get_ngo(&owner).name, boundary_200);

    // 199 'a's + 1 'Ω' (2 bytes) = 201 bytes -> rejected with NameTooLong without panicking
    let boundary_201_str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaΩ";
    assert_eq!(boundary_201_str.len(), 201);
    let boundary_201 = String::from_str(&env, boundary_201_str);
    let res_boundary = client.try_update_name(&owner, &boundary_201);
    assert_eq!(res_boundary, Err(Ok(Error::NameTooLong)));
}

#[test]
fn validate_name_bytes_rejects_invalid_utf8() {
    // 0xFF and 0xFE are never valid in UTF-8
    assert_eq!(validate_name_bytes(&[0xFF, 0xFE]), Err(Error::InvalidName));
    // Overlong NUL (0xC0 0x80)
    assert_eq!(validate_name_bytes(&[0xC0, 0x80]), Err(Error::InvalidName));
    // Lone continuation byte (0x80)
    assert_eq!(validate_name_bytes(&[0x80]), Err(Error::InvalidName));
}

#[test]
fn get_unregistered_ngo_fails() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);

    let result = client.try_get_ngo(&random);
    assert_eq!(result, Err(Ok(Error::NotRegistered)));
}

#[test]
fn is_verified_returns_false_for_registered_unverified_ngo() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    assert!(!client.is_verified(&owner));
}

#[test]
fn is_verified_returns_true_for_verified_ngo() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);

    assert!(client.is_verified(&owner));
}

#[test]
fn is_verified_returns_false_for_unknown_address() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);

    assert!(!client.is_verified(&random));
}

#[test]
fn is_verified_returns_false_for_revoked_ngo() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);
    assert!(client.is_verified(&owner));

    client.revoke_ngo(&owner);
    assert!(!client.is_verified(&owner));
}

#[test]
fn approve_ngo_marks_verified() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");
    client.register(&owner, &name);

    client.approve_ngo(&owner);

    let ngo = client.get_ngo(&owner);
    assert!(ngo.verified);
}

#[test]
fn approve_ngo_twice_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");
    client.register(&owner, &name);

    client.approve_ngo(&owner);

    let result = client.try_approve_ngo(&owner);
    assert_eq!(result, Err(Ok(Error::AlreadyVerified)));
    assert!(client.get_ngo(&owner).verified);
}

#[test]
fn approve_unregistered_ngo_fails() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);

    let result = client.try_approve_ngo(&random);
    assert_eq!(result, Err(Ok(Error::NotRegistered)));
}

// =============================================================================
// batch_approve (issue #275)
// =============================================================================

#[test]
fn batch_approve_approves_each_ngo_and_publishes_events() {
    let (env, client, _admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    client.register(&a, &String::from_str(&env, "Red Cross"));
    client.register(&b, &String::from_str(&env, "Oxfam"));
    client.register(&c, &String::from_str(&env, "Save the Children"));

    client.batch_approve(&vec![&env, a.clone(), b.clone(), c.clone()]);

    // One `approved` event per NGO, in batch order. Checked before any other
    // call, because `events().all()` only covers the most recent invocation.
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("approved"), a.clone()).into_val(&env),
                ().into_val(&env),
            ),
            (
                client.address.clone(),
                (symbol_short!("approved"), b.clone()).into_val(&env),
                ().into_val(&env),
            ),
            (
                client.address.clone(),
                (symbol_short!("approved"), c.clone()).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );

    assert!(client.get_ngo(&a).verified);
    assert!(client.get_ngo(&b).verified);
    assert!(client.get_ngo(&c).verified);
}

#[test]
fn batch_approve_reverts_atomically_on_unregistered_ngo() {
    let (env, client, _admin) = setup();
    let registered = Address::generate(&env);
    let unknown = Address::generate(&env);
    client.register(&registered, &String::from_str(&env, "Red Cross"));

    let result = client.try_batch_approve(&vec![&env, registered.clone(), unknown]);

    assert_eq!(result, Err(Ok(Error::NotRegistered)));
    // The whole batch rolls back: the registered NGO is still pending.
    assert!(!client.get_ngo(&registered).verified);
}

#[test]
fn batch_approve_reverts_atomically_on_already_verified_ngo() {
    let (env, client, _admin) = setup();
    let verified = Address::generate(&env);
    let pending = Address::generate(&env);
    client.register(&verified, &String::from_str(&env, "Red Cross"));
    client.register(&pending, &String::from_str(&env, "Oxfam"));
    client.approve_ngo(&verified);

    let result = client.try_batch_approve(&vec![&env, pending.clone(), verified.clone()]);

    assert_eq!(result, Err(Ok(Error::AlreadyVerified)));
    assert!(!client.get_ngo(&pending).verified);
    assert!(client.get_ngo(&verified).verified);
}

#[test]
fn batch_approve_rejects_a_duplicate_owner() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    let result = client.try_batch_approve(&vec![&env, owner.clone(), owner.clone()]);

    assert_eq!(result, Err(Ok(Error::AlreadyVerified)));
    assert!(!client.get_ngo(&owner).verified);
}

#[test]
fn batch_approve_empty_batch_is_a_noop() {
    let (env, client, _admin) = setup();
    let empty: Vec<Address> = Vec::new(&env);

    assert_eq!(client.try_batch_approve(&empty), Ok(Ok(())));
    assert_eq!(env.events().all().events().len(), 0);
}

#[test]
fn batch_approve_bumps_instance_and_ngo_ttl() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    age_past_thresholds(&env, &client, &owner);

    client.batch_approve(&vec![&env, owner.clone()]);

    assert_ttls_bumped(&env, &client, &owner);
}

#[test]
#[should_panic]
fn batch_approve_without_admin_auth_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    env.mock_auths(&[]);
    client.batch_approve(&vec![&env, owner]);
}

#[test]
fn revoke_ngo_clears_verified_status() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");
    client.register(&owner, &name);

    client.approve_ngo(&owner);
    assert!(client.get_ngo(&owner).verified);

    client.revoke_ngo(&owner);

    let ngo = client.get_ngo(&owner);
    assert!(!ngo.verified);
}

#[test]
fn revoke_unverified_ngo_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");
    client.register(&owner, &name);

    let result = client.try_revoke_ngo(&owner);
    assert_eq!(result, Err(Ok(Error::NotVerified)));
    assert!(!client.get_ngo(&owner).verified);
}

#[test]
fn revoke_unregistered_ngo_fails() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);

    let result = client.try_revoke_ngo(&random);
    assert_eq!(result, Err(Ok(Error::NotRegistered)));
}

#[test]
fn full_lifecycle_approve_revoke_reapprove() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");

    // 1. Register NGO
    client.register(&owner, &name);
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("register"), owner.clone()).into_val(&env),
                name.clone().into_val(&env),
            ),
        ]
    );
    let ngo = client.get_ngo(&owner);
    assert_eq!(ngo.owner, owner);
    assert_eq!(ngo.name, name);
    assert!(!ngo.verified);

    // 2. Approve NGO
    client.approve_ngo(&owner);
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("approved"), owner.clone()).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
    let ngo = client.get_ngo(&owner);
    assert_eq!(ngo.owner, owner);
    assert_eq!(ngo.name, name);
    assert!(ngo.verified);

    // 3. Revoke NGO
    client.revoke_ngo(&owner);
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("revoked"), owner.clone()).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
    let ngo = client.get_ngo(&owner);
    assert_eq!(ngo.owner, owner);
    assert_eq!(ngo.name, name);
    assert!(!ngo.verified);

    // 4. Reapprove NGO
    client.approve_ngo(&owner);
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("approved"), owner.clone()).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
    let ngo = client.get_ngo(&owner);
    assert_eq!(ngo.owner, owner);
    assert_eq!(ngo.name, name);
    assert!(ngo.verified);
}

#[test]
fn touch_ngo_leaves_entry_unchanged() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");
    client.register(&owner, &name);
    client.approve_ngo(&owner);

    let before = client.get_ngo(&owner);
    client.touch_ngo(&owner);
    let after = client.get_ngo(&owner);

    assert_eq!(before, after);
}

#[test]
fn touch_unregistered_ngo_fails() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);

    let result = client.try_touch_ngo(&random);
    assert_eq!(result, Err(Ok(Error::NotRegistered)));
}

fn instance_ttl(env: &Env, client: &NgoRegistryClient) -> u32 {
    env.as_contract(&client.address, || env.storage().instance().get_ttl())
}

fn ngo_ttl(env: &Env, client: &NgoRegistryClient, owner: &Address) -> u32 {
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::Ngo(owner.clone()))
    })
}

/// Moves the ledger forward two days, which drops both the instance and a
/// freshly bumped NGO entry below their lifetime thresholds. Asserts that
/// it did, so a test calling this can't pass just because nothing needed
/// a bump.
fn age_past_thresholds(env: &Env, client: &NgoRegistryClient, owner: &Address) {
    env.ledger()
        .with_mut(|l| l.sequence_number += 2 * DAY_IN_LEDGERS);
    assert!(instance_ttl(env, client) < INSTANCE_LIFETIME_THRESHOLD);
    assert!(ngo_ttl(env, client, owner) < NGO_LIFETIME_THRESHOLD);
}

fn assert_ttls_bumped(env: &Env, client: &NgoRegistryClient, owner: &Address) {
    assert_eq!(instance_ttl(env, client), INSTANCE_BUMP_AMOUNT);
    assert_eq!(ngo_ttl(env, client, owner), NGO_BUMP_AMOUNT);
}

#[test]
fn register_bumps_instance_and_ngo_ttl() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    assert_ttls_bumped(&env, &client, &owner);
}

#[test]
fn approve_ngo_bumps_instance_and_ngo_ttl() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    age_past_thresholds(&env, &client, &owner);

    client.approve_ngo(&owner);

    assert_ttls_bumped(&env, &client, &owner);
}

#[test]
fn revoke_ngo_bumps_instance_and_ngo_ttl() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);
    age_past_thresholds(&env, &client, &owner);

    client.revoke_ngo(&owner);

    assert_ttls_bumped(&env, &client, &owner);
}

#[test]
fn update_name_changes_name_before_approval() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Crsos"));

    let fixed = String::from_str(&env, "Red Cross");
    client.update_name(&owner, &fixed);

    // Events cover only the latest top-level call, so read them before
    // `get_ngo` below replaces them.
    let events = env.events().all();
    let expected: Vec<(Address, Vec<Val>, Val)> = vec![
        &env,
        (
            client.address.clone(),
            (symbol_short!("renamed"), owner.clone()).into_val(&env),
            fixed.into_val(&env),
        ),
    ];
    assert_eq!(events, expected);

    assert_eq!(
        env.events().all(),
        soroban_sdk::vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("renamed"), owner.clone()).into_val(&env),
                fixed.into_val(&env)
            ),
        ]
    );

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("renamed"), owner.clone()).into_val(&env),
                fixed.into_val(&env),
            ),
        ]
    );

    assert_eq!(
        client.get_ngo(&owner),
        Ngo {
            owner: owner.clone(),
            name: fixed.clone(),
            verified: false,
        }
    );
}

#[test]
fn update_name_after_approval_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");
    client.register(&owner, &name);
    client.approve_ngo(&owner);

    let result = client.try_update_name(&owner, &String::from_str(&env, "Blue Cross"));
    assert_eq!(result, Err(Ok(Error::AlreadyVerified)));
    assert_eq!(client.get_ngo(&owner).name, name);
}

#[test]
fn update_name_after_revocation_succeeds() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Crsos"));
    client.approve_ngo(&owner);

    // The name an admin approved is locked only while the NGO stays
    // verified, so the rename has to be rejected at this point.
    let blocked = client.try_update_name(&owner, &String::from_str(&env, "Red Cross"));
    assert_eq!(blocked, Err(Ok(Error::AlreadyVerified)));

    client.revoke_ngo(&owner);
    assert!(!client.get_ngo(&owner).verified);

    let fixed = String::from_str(&env, "Red Cross");
    client.update_name(&owner, &fixed);

    // Events cover only the latest top-level call, so read them before
    // `get_ngo` below replaces them.
    assert_eq!(
        env.events().all(),
        soroban_sdk::vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("renamed"), owner.clone()).into_val(&env),
                fixed.clone().into_val(&env),
            ),
        ]
    );

    assert_eq!(
        client.get_ngo(&owner),
        Ngo {
            owner: owner.clone(),
            name: fixed,
            verified: false,
        }
    );
}

#[test]
fn update_name_for_unregistered_ngo_fails() {
    let (env, client, _admin) = setup();
    let random = Address::generate(&env);

    let result = client.try_update_name(&random, &String::from_str(&env, "Red Cross"));
    assert_eq!(result, Err(Ok(Error::NotRegistered)));
}

#[test]
fn update_name_requires_owner_auth() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Crsos"));

    client.update_name(&owner, &String::from_str(&env, "Red Cross"));

    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    let (address, invocation) = &auths[0];
    assert_eq!(address, &owner);
    match &invocation.function {
        AuthorizedFunction::Contract((contract, function, _)) => {
            assert_eq!(contract, &client.address);
            assert_eq!(function, &Symbol::new(&env, "update_name"));
        }
        _ => panic!("expected a contract invocation"),
    }
}

#[test]
fn update_name_bumps_instance_and_ngo_ttl() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Crsos"));
    age_past_thresholds(&env, &client, &owner);

    client.update_name(&owner, &String::from_str(&env, "Red Cross"));

    assert_ttls_bumped(&env, &client, &owner);
}

#[test]
fn touch_ngo_bumps_instance_and_ngo_ttl() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    age_past_thresholds(&env, &client, &owner);

    client.touch_ngo(&owner);

    assert_ttls_bumped(&env, &client, &owner);
}

// --- Event assertion tests (issue #62) ---

#[test]
fn register_publishes_event() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Red Cross");

    client.register(&owner, &name);

    assert_eq!(
        env.events().all(),
        soroban_sdk::vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("register"), owner).into_val(&env),
                name.into_val(&env),
            ),
        ]
    );
}

#[test]
fn approve_ngo_publishes_event() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    client.approve_ngo(&owner);

    // Events are cleared per top-level call; only the approved event is visible.
    assert_eq!(
        env.events().all(),
        soroban_sdk::vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("approved"), owner).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
}

#[test]
fn revoke_ngo_publishes_event() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);

    client.revoke_ngo(&owner);

    // Events are cleared per top-level call; only the revoked event is visible.
    assert_eq!(
        env.events().all(),
        soroban_sdk::vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("revoked"), owner).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
}

// =============================================================================
// upgrade (contract bytecode upgrade)
// =============================================================================

#[test]
#[ignore = "requires an uploaded wasm hash; upgrade panics before auth is observable"]
fn upgrade_requires_admin_auth() {
    let (env, client, admin) = setup();
    let fake_hash = BytesN::from_array(&env, &[1u8; 32]);

    client.upgrade(&fake_hash);
    let auths = env.auths();
    assert_eq!(auths.len(), 1, "expected exactly one authorizer");
    assert_eq!(auths[0].0, admin);
    match &auths[0].1.function {
        AuthorizedFunction::Contract((contract, function, _)) => {
            assert_eq!(contract, &client.address);
            assert_eq!(function, &Symbol::new(&env, "upgrade"));
        }
        _ => panic!("expected a contract invocation"),
    }
}

#[test]
fn upgrade_rejects_non_admin() {
    let (env, client, _admin) = setup();
    let fake_hash = BytesN::from_array(&env, &[1u8; 32]);

    env.mock_auths(&[]);
    let result = client.try_upgrade(&fake_hash);
    assert!(result.is_err());
}

#[test]
fn upgrade_fails_with_unuploaded_wasm_hash() {
    let (env, client, _admin) = setup();
    let unuploaded_hash = BytesN::from_array(&env, &[42u8; 32]);
    let result = client.try_upgrade(&unuploaded_hash);
    assert!(result.is_err());
}

#[test]
fn upgrade_preserves_state() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Doctors Without Borders");

    client.register(&owner, &name);
    client.approve_ngo(&owner);

    let ngo_before = client.get_ngo(&owner);
    assert!(ngo_before.verified);
    assert_eq!(ngo_before.name, name);
    assert_eq!(client.ngo_count(), 1);

    // Negative case: attempting an upgrade with an unuploaded wasm hash fails
    let unuploaded_hash = BytesN::from_array(&env, &[99u8; 32]);
    assert!(client.try_upgrade(&unuploaded_hash).is_err());

    // State reads back identically after
    let ngo_after = client.get_ngo(&owner);
    assert_eq!(ngo_before.owner, ngo_after.owner);
    assert_eq!(ngo_before.name, ngo_after.name);
    assert_eq!(ngo_before.verified, ngo_after.verified);
    assert_eq!(client.ngo_count(), 1);
}

#[test]
fn upgrade_publishes_event() {
    let (env, client, _admin) = setup();
    // Upload any bytes so the upgrade target exists; the host only checks
    // that the hash is present, not that it is executable.
    let new_wasm_hash = env.deployer().upload_contract_wasm(Bytes::new(&env));

    client.upgrade(&new_wasm_hash);

    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("upgrade"),).into_val(&env),
                new_wasm_hash.into_val(&env),
            ),
        ]
    );
}

// list_ngos pagination tests (issue #283)
// =============================================================================

#[test]
fn list_ngos_empty_registry_returns_empty_list() {
    let (env, client, _admin) = setup();
    let result = client.list_ngos(&0, &10);
    assert_eq!(result.len(), 0);
    assert_eq!(result, Vec::new(&env));
}

#[test]
fn list_ngos_one_ngo() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Only NGO"));

    let result = client.list_ngos(&0, &10);
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(0).unwrap(), owner);
}

#[test]
fn list_ngos_across_page_boundaries() {
    let (env, client, _admin) = setup();
    let mut owners = soroban_sdk::vec![&env];
    for _ in 0..5 {
        let owner = Address::generate(&env);
        let name = String::from_str(&env, "NGO");
        client.register(&owner, &name);
        owners.push_back(owner);
    }

    // Page 1: start = 0, limit = 2 -> owners [0, 1]
    let page1 = client.list_ngos(&0, &2);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap(), owners.get(0).unwrap());
    assert_eq!(page1.get(1).unwrap(), owners.get(1).unwrap());

    // Page 2: start = 2, limit = 2 -> owners [2, 3]
    let page2 = client.list_ngos(&2, &2);
    assert_eq!(page2.len(), 2);
    assert_eq!(page2.get(0).unwrap(), owners.get(2).unwrap());
    assert_eq!(page2.get(1).unwrap(), owners.get(3).unwrap());

    // Page 3: start = 4, limit = 2 -> owners [4]
    let page3 = client.list_ngos(&4, &2);
    assert_eq!(page3.len(), 1);
    assert_eq!(page3.get(0).unwrap(), owners.get(4).unwrap());
}

#[test]
fn list_ngos_start_beyond_end_returns_empty_list() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "NGO"));

    let result = client.list_ngos(&10, &5);
    assert_eq!(result.len(), 0);
}

#[test]
fn list_ngos_limit_larger_than_cap_is_clamped_to_max_limit() {
    let (env, client, _admin) = setup();
    // Register 55 NGOs (> MAX_LIST_LIMIT = 50)
    let mut registered = soroban_sdk::vec![&env];
    for _ in 0..55 {
        let owner = Address::generate(&env);
        client.register(&owner, &String::from_str(&env, "NGO"));
        registered.push_back(owner);
    }

    // Request limit = 100, must be clamped to 50
    let page = client.list_ngos(&0, &100);
    assert_eq!(page.len(), MAX_LIST_LIMIT);
    for i in 0..MAX_LIST_LIMIT {
        assert_eq!(page.get(i).unwrap(), registered.get(i).unwrap());
    }

    // Remaining 5 can be fetched in next page
    let remaining = client.list_ngos(&MAX_LIST_LIMIT, &50);
    assert_eq!(remaining.len(), 5);
    for i in 0..5 {
        assert_eq!(
            remaining.get(i).unwrap(),
            registered.get(MAX_LIST_LIMIT + i).unwrap()
        );
    }
}

#[test]
fn list_ngos_unregister_removes_from_middle_and_maintains_index() {
    let (env, client, _admin) = setup();
    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let owner_c = Address::generate(&env);

    client.register(&owner_a, &String::from_str(&env, "NGO A"));
    client.register(&owner_b, &String::from_str(&env, "NGO B"));
    client.register(&owner_c, &String::from_str(&env, "NGO C"));

    let initial = client.list_ngos(&0, &10);
    assert_eq!(initial.len(), 3);
    assert_eq!(initial.get(0).unwrap(), owner_a);
    assert_eq!(initial.get(1).unwrap(), owner_b);
    assert_eq!(initial.get(2).unwrap(), owner_c);

    // Unregister B (middle of list). Swap-and-pop moves C into position 1.
    client.unregister(&owner_b);

    let after_unregister = client.list_ngos(&0, &10);
    assert_eq!(after_unregister.len(), 2);
    assert_eq!(after_unregister.get(0).unwrap(), owner_a);
    assert_eq!(after_unregister.get(1).unwrap(), owner_c);

    // B must not be listed
    assert_eq!(client.try_get_ngo(&owner_b), Err(Ok(Error::NotRegistered)));
}

#[test]
fn list_ngos_re_registering_after_unregister_has_no_duplicates() {
    let (env, client, _admin) = setup();
    let owner_a = Address::generate(&env);
    let owner_b = Address::generate(&env);
    let owner_c = Address::generate(&env);

    client.register(&owner_a, &String::from_str(&env, "NGO A"));
    client.register(&owner_b, &String::from_str(&env, "NGO B"));
    client.register(&owner_c, &String::from_str(&env, "NGO C"));

    // Unregister B
    client.unregister(&owner_b);

    // Re-register B
    client.register(&owner_b, &String::from_str(&env, "NGO B New"));

    let result = client.list_ngos(&0, &10);
    assert_eq!(result.len(), 3);
    assert_eq!(result.get(0).unwrap(), owner_a);
    assert_eq!(result.get(1).unwrap(), owner_c);
    assert_eq!(result.get(2).unwrap(), owner_b);
}

#[test]
fn list_ngos_unregister_last_and_only_entry() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Solo NGO"));

    assert_eq!(client.list_ngos(&0, &10).len(), 1);

    client.unregister(&owner);

    assert_eq!(client.list_ngos(&0, &10).len(), 0);
}

// =============================================================================
// register and unregister auth and events (issue #294)
// =============================================================================

/// Asserts that the last call recorded exactly one authorization: `expected`
/// authorizing `function` on the registry itself.
fn assert_sole_auth(env: &Env, client: &NgoRegistryClient, expected: &Address, function: &str) {
    let auths = env.auths();
    assert_eq!(auths.len(), 1, "expected exactly one authorizer");
    let (address, invocation) = &auths[0];
    assert_eq!(address, expected);
    match &invocation.function {
        AuthorizedFunction::Contract((contract, name, _)) => {
            assert_eq!(contract, &client.address);
            assert_eq!(name, &Symbol::new(env, function));
        }
        _ => panic!("expected a contract invocation"),
    }
}

#[test]
fn register_requires_owner_auth() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    client.register(&owner, &String::from_str(&env, "Red Cross"));

    assert_sole_auth(&env, &client, &owner, "register");
}

#[test]
#[should_panic]
fn register_without_owner_auth_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    env.mock_auths(&[]);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
}

#[test]
fn unregister_requires_owner_auth() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    client.unregister(&owner);

    assert_sole_auth(&env, &client, &owner, "unregister");
}

#[test]
#[should_panic]
fn unregister_without_owner_auth_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    env.mock_auths(&[]);
    client.unregister(&owner);
}

#[test]
fn unregister_publishes_event() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    client.unregister(&owner);

    // Events are cleared per top-level call; only the unregist event is visible.
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("unregist"), owner).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
}

#[test]
fn unregister_verified_ngo_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);

    let result = client.try_unregister(&owner);

    assert_eq!(result, Err(Ok(Error::AlreadyVerified)));
    // The entry survives the rejected call, verification intact.
    assert!(client.get_ngo(&owner).verified);
    assert_eq!(client.list_ngos(&0, &10).len(), 1);
}

// =============================================================================
// remove_ngo (issue #282)
// =============================================================================

#[test]
fn remove_ngo_deletes_unverified_entry() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    assert_eq!(client.total_ngos(), 1);

    client.remove_ngo(&owner);

    assert_eq!(client.try_get_ngo(&owner), Err(Ok(Error::NotRegistered)));
    assert!(!client.is_verified(&owner));
    assert_eq!(client.total_ngos(), 0);
    assert_eq!(client.list_ngos(&0, &10).len(), 0);
}

#[test]
fn remove_ngo_publishes_event() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    client.remove_ngo(&owner);

    // Events are cleared per top-level call; only the removed event is visible.
    assert_eq!(
        env.events().all(),
        vec![
            &env,
            (
                client.address.clone(),
                (symbol_short!("removed"), owner).into_val(&env),
                ().into_val(&env),
            ),
        ]
    );
}

#[test]
fn remove_ngo_rejects_a_verified_ngo() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);

    let result = client.try_remove_ngo(&owner);

    assert_eq!(result, Err(Ok(Error::AlreadyVerified)));
    // Still there, still verified: revoke_ngo is the way to undo an approval.
    assert!(client.get_ngo(&owner).verified);
    assert_eq!(client.total_ngos(), 1);
}

#[test]
fn remove_ngo_after_revocation_succeeds() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));
    client.approve_ngo(&owner);
    client.revoke_ngo(&owner);

    client.remove_ngo(&owner);

    assert_eq!(client.try_get_ngo(&owner), Err(Ok(Error::NotRegistered)));
    assert_eq!(client.total_ngos(), 0);
}

#[test]
fn remove_ngo_unknown_owner_fails() {
    let (env, client, _admin) = setup();
    let unknown = Address::generate(&env);

    let result = client.try_remove_ngo(&unknown);

    assert_eq!(result, Err(Ok(Error::NotRegistered)));
}

#[test]
fn remove_ngo_requires_admin_auth() {
    let (env, client, admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    client.remove_ngo(&owner);

    assert_sole_auth(&env, &client, &admin, "remove_ngo");
}

#[test]
#[should_panic]
fn remove_ngo_without_admin_auth_fails() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    client.register(&owner, &String::from_str(&env, "Red Cross"));

    env.mock_auths(&[]);
    client.remove_ngo(&owner);
}

#[test]
fn remove_ngo_frees_the_index_and_lets_the_owner_register_again() {
    let (env, client, _admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    client.register(&a, &String::from_str(&env, "A"));
    client.register(&b, &String::from_str(&env, "B"));
    client.register(&c, &String::from_str(&env, "C"));

    // Removing the middle entry swaps the last one into its slot.
    client.remove_ngo(&b);

    let owners = client.list_ngos(&0, &10);
    assert_eq!(owners.len(), 2);
    assert_eq!(owners.get(0).unwrap(), a);
    assert_eq!(owners.get(1).unwrap(), c);
    assert_eq!(client.total_ngos(), 2);

    // The freed owner can apply again from scratch.
    client.register(&b, &String::from_str(&env, "B again"));
    assert!(!client.get_ngo(&b).verified);
    assert_eq!(client.total_ngos(), 3);
}

// =============================================================================
// NgoCount accuracy tests (issue #265)
// =============================================================================

#[test]
fn register_unregister_register_maintains_count() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "Test NGO");

    assert_eq!(client.ngo_count(), 0);

    client.register(&owner, &name);
    assert_eq!(client.ngo_count(), 1);

    client.unregister(&owner);
    assert_eq!(client.ngo_count(), 0);

    client.register(&owner, &name);
    assert_eq!(client.ngo_count(), 1);
}

#[test]
fn two_registrations_one_unregistration_count() {
    let (env, client, _admin) = setup();
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);

    client.register(&owner1, &String::from_str(&env, "NGO 1"));
    assert_eq!(client.ngo_count(), 1);

    client.register(&owner2, &String::from_str(&env, "NGO 2"));
    assert_eq!(client.ngo_count(), 2);

    client.unregister(&owner1);
    assert_eq!(client.ngo_count(), 1);

    client.remove_ngo(&owner2);
    assert_eq!(client.ngo_count(), 0);
}

#[test]
fn count_stays_consistent_after_deterministic_operations() {
    let (env, client, _admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    let d = Address::generate(&env);

    assert_eq!(client.ngo_count(), 0);

    // Register a -> 1
    client.register(&a, &String::from_str(&env, "A"));
    assert_eq!(client.ngo_count(), 1);

    // Register b -> 2
    client.register(&b, &String::from_str(&env, "B"));
    assert_eq!(client.ngo_count(), 2);

    // Register c -> 3
    client.register(&c, &String::from_str(&env, "C"));
    assert_eq!(client.ngo_count(), 3);

    // Unregister b -> 2
    client.unregister(&b);
    assert_eq!(client.ngo_count(), 2);

    // Register d -> 3
    client.register(&d, &String::from_str(&env, "D"));
    assert_eq!(client.ngo_count(), 3);

    // Admin removes c -> 2
    client.remove_ngo(&c);
    assert_eq!(client.ngo_count(), 2);

    // Register b again -> 3
    client.register(&b, &String::from_str(&env, "B again"));
    assert_eq!(client.ngo_count(), 3);

    // Unregister a -> 2
    client.unregister(&a);
    assert_eq!(client.ngo_count(), 2);

    // Admin removes d -> 1
    client.remove_ngo(&d);
    assert_eq!(client.ngo_count(), 1);

    // Unregister b -> 0
    client.unregister(&b);
    assert_eq!(client.ngo_count(), 0);
}

#[test]
fn unregister_unknown_owner_does_not_change_count() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let unknown = Address::generate(&env);

    client.register(&owner, &String::from_str(&env, "Known NGO"));
    assert_eq!(client.ngo_count(), 1);

    let res_unreg = client.try_unregister(&unknown);
    assert_eq!(res_unreg, Err(Ok(Error::NotRegistered)));
    assert_eq!(client.ngo_count(), 1);

    let res_rem = client.try_remove_ngo(&unknown);
    assert_eq!(res_rem, Err(Ok(Error::NotRegistered)));
    assert_eq!(client.ngo_count(), 1);
}

#[test]
fn register_overflow_returns_arithmetic_overflow() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    // Set stored NgoCount to u64::MAX
    env.as_contract(&client.address, || {
        env.storage().instance().set(&DataKey::NgoCount, &u64::MAX);
    });

    let res = client.try_register(&owner, &String::from_str(&env, "Overflow NGO"));
    assert_eq!(res, Err(Ok(Error::ArithmeticOverflow)));
    assert_eq!(client.try_get_ngo(&owner), Err(Ok(Error::NotRegistered)));
}

#[test]
fn decrement_at_zero_returns_arithmetic_overflow() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    client.register(&owner, &String::from_str(&env, "Test NGO"));
    assert_eq!(client.ngo_count(), 1);

    // Corrupt NgoCount to 0 in instance storage
    env.as_contract(&client.address, || {
        env.storage().instance().set(&DataKey::NgoCount, &0u64);
    });

    let res = client.try_unregister(&owner);
    assert_eq!(res, Err(Ok(Error::ArithmeticOverflow)));

    // Entry should survive because the operation aborted before deletion
    assert!(client.try_get_ngo(&owner).is_ok());

    let res_rem = client.try_remove_ngo(&owner);
    assert_eq!(res_rem, Err(Ok(Error::ArithmeticOverflow)));
    assert!(client.try_get_ngo(&owner).is_ok());
}

#[test]
fn failed_register_leaves_count_unchanged() {
    let (env, client, _admin) = setup();
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);

    client.register(&owner1, &String::from_str(&env, "NGO 1"));
    assert_eq!(client.ngo_count(), 1);

    // Double register fails
    let res_double = client.try_register(&owner1, &String::from_str(&env, "NGO 1 again"));
    assert_eq!(res_double, Err(Ok(Error::AlreadyRegistered)));
    assert_eq!(client.ngo_count(), 1);

    // Invalid name fails
    let res_inv = client.try_register(&owner2, &String::from_str(&env, "   "));
    assert_eq!(res_inv, Err(Ok(Error::InvalidName)));
    assert_eq!(client.ngo_count(), 1);

    // Name too long fails
    let too_long = String::from_str(&env, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let res_long = client.try_register(&owner2, &too_long);
    assert_eq!(res_long, Err(Ok(Error::NameTooLong)));
    assert_eq!(client.ngo_count(), 1);
}

// =============================================================================
// total_ngos alias tests (issue #264)
// =============================================================================

#[test]
fn total_ngos_alias_agrees_with_ngo_count() {
    let (env, client, _admin) = setup();
    let a = Address::generate(&env);
    let b = Address::generate(&env);

    // Initial state: both agree at zero
    assert_eq!(client.ngo_count(), 0);
    assert_eq!(client.total_ngos(), 0);

    // Register two NGOs: both getters agree
    client.register(&a, &String::from_str(&env, "NGO A"));
    assert_eq!(client.ngo_count(), 1);
    assert_eq!(client.total_ngos(), 1);

    client.register(&b, &String::from_str(&env, "NGO B"));
    assert_eq!(client.ngo_count(), 2);
    assert_eq!(client.total_ngos(), 2);

    // Unregister sequence: both getters agree after removal
    client.unregister(&a);
    assert_eq!(client.ngo_count(), 1);
    assert_eq!(client.total_ngos(), 1);

    // Admin removal: both getters agree back to zero
    client.remove_ngo(&b);
    assert_eq!(client.ngo_count(), 0);
    assert_eq!(client.total_ngos(), 0);
}

#[test]
fn total_ngos_alias_register_unregister_register_sequence() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "NGO Test");

    assert_eq!(client.total_ngos(), 0);
    assert_eq!(client.ngo_count(), 0);

    client.register(&owner, &name);
    assert_eq!(client.total_ngos(), 1);
    assert_eq!(client.ngo_count(), 1);

    client.unregister(&owner);
    assert_eq!(client.total_ngos(), 0);
    assert_eq!(client.ngo_count(), 0);

    client.register(&owner, &name);
    assert_eq!(client.total_ngos(), 1);
    assert_eq!(client.ngo_count(), 1);
}
