extern crate std;

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Events as _;
use soroban_sdk::Env;
use soroban_sdk::{token, vec, IntoVal, Symbol};

fn create_usdc<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    let address = contract_address.address();
    let client = token::Client::new(env, &address);
    let admin_client = token::StellarAssetClient::new(env, &address);
    (address, client, admin_client)
}

fn create_vault(env: &Env) -> (Address, CalloraVaultClient<'_>) {
    let address = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(env, &address);
    (address, client)
}

fn fund_vault(
    usdc_admin_client: &token::StellarAssetClient,
    vault_address: &Address,
    amount: i128,
) {
    usdc_admin_client.mint(vault_address, &amount);
}

fn fund_user(usdc_admin_client: &token::StellarAssetClient, user: &Address, amount: i128) {
    usdc_admin_client.mint(user, &amount);
}

/// Approve spender to transfer amount from from (for deposit tests; from must have auth).
fn approve_spend(
    _env: &Env,
    usdc_client: &token::Client,
    from: &Address,
    spender: &Address,
    amount: i128,
) {
    // expiration_ledger 0 = no expiration in Stellar Asset Contract
    usdc_client.approve(from, spender, &amount, &0u32);
}
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{IntoVal, Symbol};

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    // Call init directly inside as_contract so events are captured
    let events = env.as_contract(&contract_id, || {
        CalloraVault::init(env.clone(), owner.clone(), Some(1000));
        env.events().all()
    });

    // Verify balance through client
    let client = CalloraVaultClient::new(&env, &contract_id);
    assert_eq!(client.balance(), 1000);

    // Verify "init" event was emitted
    let last_event = events.last().expect("expected at least one event");

    // Contract ID matches
    assert_eq!(last_event.0, contract_id);

    // Topic 0 = Symbol("init"), Topic 1 = owner address
    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "init"));
    assert_eq!(topic1, owner);

    // Data = initial balance as i128
    let data: i128 = last_event.2.into_val(&env);
    assert_eq!(data, 1000);
}

#[test]
fn init_default_zero_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc, &Some(1000), &None, &None, &None);
    let _events = env.events().all();

    client.init(&owner, &None);
    assert_eq!(client.balance(), 0);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();
    client.deposit(&owner, &200);
    assert_eq!(client.balance(), 300);

    client.deduct(&owner, &50);
    assert_eq!(client.balance(), 250);
}

#[test]
fn owner_can_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    // Initialize vault with initial balance
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_address, &Some(500), &None, &None, &None);

    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after init");
    assert_eq!(meta.owner, owner, "owner changed after init");
    assert_eq!(balance, 500, "incorrect balance after init");

    fund_user(&usdc_admin, &owner, 575);
    approve_spend(&env, &usdc_client, &owner, &contract_id, 575);
    client.deposit(&owner, &300);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deposit");
    assert_eq!(balance, 800, "incorrect balance after deposit");

    // Deduct and verify consistency
    client.deduct(&owner, &150, &None);
    client.deduct(&owner, &150, &None);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deduct");
    assert_eq!(balance, 500, "incorrect balance after deduct");

    // Perform multiple operations and verify final state
    client.deposit(&owner, &100);
    client.deduct(&owner, &50, &None);
    client.deposit(&owner, &25);
    client.deposit(&owner, &100);
    client.deduct(&owner, &50, &None);
    client.deposit(&owner, &25);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(
        meta.balance, balance,
        "balance mismatch after multiple operations"
    );
    assert_eq!(balance, 650, "incorrect final balance");
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exact_balance_and_panic() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let (contract_id, client) = create_vault(&env);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    assert_eq!(client.balance(), 100);

    // Deduct exact balance
    client.deduct(&owner, &100, &None);
    assert_eq!(client.balance(), 0);

    // Further deduct should panic
    client.deduct(&owner, &1, &None);
    client.init(&owner, &Some(100));

    // Mock the owner as the invoker
    env.mock_all_auths();
    client.deposit(&owner, &200);

    assert_eq!(client.balance(), 300);
}

#[test]
fn allowed_depositor_can_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_address, &Some(1000), &None, &None, &None);
    let caller = Address::generate(&env);
    let req_id = Symbol::new(&env, "req123");

    // Call client directly to avoid re-entry panic inside as_contract
    client.deduct(&caller, &200, &Some(req_id.clone()));
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Owner sets the allowed depositor
    env.mock_all_auths();
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));

    // Depositor can now deposit
    client.deposit(&depositor, &50);
    assert_eq!(client.balance(), 150);
}

#[test]
#[should_panic(expected = "unauthorized: only owner or allowed depositor can deposit")]
fn unauthorized_address_cannot_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Try to deposit as unauthorized address (should panic)
    env.mock_all_auths();
    let unauthorized_addr = Address::generate(&env);
    client.deposit(&unauthorized_addr, &50);
}

#[test]
fn owner_can_set_allowed_depositor() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Owner sets allowed depositor
    env.mock_all_auths();
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));

    // Depositor can deposit
    client.deposit(&depositor, &25);
    assert_eq!(client.balance(), 125);
}

#[test]
fn owner_can_clear_allowed_depositor() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

#[test]
fn deduct_returns_new_balance() {
    let env = Env::default();
    env.mock_all_auths();

    // Set depositor
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));
    client.deposit(&depositor, &50);
    assert_eq!(client.balance(), 150);

    fund_vault(&usdc_admin, &vault_address, 100);
    vault.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    let new_balance = vault.deduct(&owner, &30, &None);
    assert_eq!(new_balance, 70);
    assert_eq!(vault.balance(), 70);
}
    // Clear depositor
    client.set_allowed_depositor(&owner, &None);

    // Depositor can no longer deposit (would panic if attempted)
    // Owner can still deposit
    client.deposit(&owner, &25);
    assert_eq!(client.balance(), 175);
}

#[test]
#[should_panic(expected = "unauthorized: owner only")]
fn non_owner_cannot_set_allowed_depositor() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Try to set allowed depositor as non-owner (should panic)
    env.mock_all_auths();
    let non_owner_addr = Address::generate(&env);
    client.set_allowed_depositor(&non_owner_addr, &Some(depositor));
}

#[test]
#[should_panic(expected = "unauthorized: only owner or allowed depositor can deposit")]
fn deposit_after_depositor_cleared_is_rejected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set and then clear depositor
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));
    client.set_allowed_depositor(&owner, &None);

    // Depositor should no longer be able to deposit
    client.deposit(&depositor, &50);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deposit_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_address, &Some(1000), &None, &None, &None);
    client.deposit(&owner, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deposit_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    client.deposit(&owner, &-100);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_address, &Some(500), &None, &None, &None);
    client.deduct(&owner, &0, &None);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    client.deduct(&owner, &-50, &None);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exceeds_balance_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 50);
    client.init(&owner, &usdc_address, &Some(50), &None, &None, &None);
    client.deduct(&owner, &100, &None);
}
fn test_transfer_ownership() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // transfer ownership via client
    client.transfer_ownership(&new_owner);

    let transfer_event = env
        .events()
        .all()
        .into_iter()
        .find(|e| {
            e.0 == contract_id && {
                let topics = &e.1;
                if !topics.is_empty() {
                    let topic_name: Symbol = topics.get(0).unwrap().into_val(&env);
                    topic_name == Symbol::new(&env, "transfer_ownership")
                } else {
                    false
                }
            }
        })
        .expect("expected transfer event");

    let topics = &transfer_event.1;
    let topic_old_owner: Address = topics.get(1).unwrap().into_val(&env);
    assert!(topic_old_owner == owner);

    let topic_new_owner: Address = topics.get(2).unwrap().into_val(&env);
    assert!(topic_new_owner == new_owner);
}

#[test]
#[should_panic(expected = "new_owner must be different from current owner")]
fn test_transfer_ownership_same_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // This should panic because new_owner is the same as current owner
    client.transfer_ownership(&owner);
}

#[test]
#[should_panic]
fn test_transfer_ownership_not_owner() {
    let env = Env::default();

    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let _not_owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    // Mock auth for init
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &owner,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "init",
            args: (&owner, &Some(100i128)).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);

    client.withdraw(&50);
}

#[test]
#[should_panic(expected = "vault already initialized")]
fn init_already_initialized_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    client.init(&owner, &usdc_address, &Some(200), &None, &None, &None); // Should panic
}

/// Fuzz test: random deposit/deduct sequence asserting balance >= 0 and matches expected.
/// Run with: cargo test --package callora-vault fuzz_deposit_and_deduct -- --nocapture
#[test]
fn fuzz_deposit_and_deduct() {
    use rand::Rng;

    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    let initial_balance: i128 = 1_000;
    fund_vault(&usdc_admin, &vault_address, initial_balance);
    vault.init(&owner, &usdc_address, &Some(initial_balance), &None, &None);

    fund_user(&usdc_admin, &owner, 250_000);
    approve_spend(&env, &usdc_client, &owner, &vault_address, 250_000);

    let mut expected = initial_balance;
    let mut rng = rand::thread_rng();

    for _ in 0..500 {
        if rng.gen_bool(0.5) {
            let amount = rng.gen_range(1..=500);
            vault.deposit(&owner, &amount);
            expected += amount;
        } else if expected > 0 {
            let amount = rng.gen_range(1..=expected.min(500));
            vault.deduct(&owner, &amount, &None);
            expected -= amount;
        }

        let balance = vault.balance();
        assert!(balance >= 0, "balance went negative: {}", balance);
        assert_eq!(
            balance, expected,
            "balance mismatch: got {}, expected {}",
            balance, expected
        );
    }

    assert_eq!(vault.balance(), expected);
}

// #[test]
// fn deduct_returns_new_balance() {
//     let env = Env::default();
//     env.mock_all_auths();

//     let owner = Address::generate(&env);
//     let (_, vault) = create_vault(&env);
//     let (usdc_address, _, _) = create_usdc(&env, &owner);

//     vault.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
//     let new_balance = vault.deduct(&owner, &30, &None);
//     assert_eq!(new_balance, 70);
//     assert_eq!(vault.balance(), 70);
// }

#[test]
fn batch_deduct_all_succeed() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 60);
    client.init(&owner, &usdc_address, &Some(60), &None, &None, &None);
    let items = vec![
        &env,
        DeductItem {
            amount: 10,
            request_id: None,
        },
        DeductItem {
            amount: 20,
            request_id: None,
        },
        DeductItem {
            amount: 30,
            request_id: None,
        },
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();
    let new_balance = client.batch_deduct(&caller, &items);
    assert_eq!(new_balance, 0);
    assert_eq!(client.balance(), 0);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn batch_deduct_all_revert() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 25);
    client.init(&owner, &usdc_address, &Some(25), &None, &None, &None);
    let items = vec![
        &env,
        DeductItem {
            amount: 60,
            request_id: None,
        },
        DeductItem {
            amount: 60,
            request_id: None,
        }, // total 120 > 100
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();
    client.batch_deduct(&caller, &items);
}

#[test]
fn test_concurrent_deposits() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_address, 100);
    vault.init(&owner, &usdc_address, &Some(100), &None, &None, &None);

    let dep1 = Address::generate(&env);
    let dep2 = Address::generate(&env);

    fund_user(&usdc_admin, &dep1, 200);
    fund_user(&usdc_admin, &dep2, 300);

    // Approve the vault to spend on behalf of depositors
    approve_spend(&env, &usdc_client, &dep1, &vault_address, 200);
    approve_spend(&env, &usdc_client, &dep2, &vault_address, 300);

    // Concurrent deposits
    vault.deposit(&dep1, &200);
    vault.deposit(&dep2, &300);

    assert_eq!(vault.balance(), 600);
}

#[test]
fn init_twice_panics_on_reinit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 50);
    client.init(&owner, &usdc_address, &Some(25), &None, &None, &None);
    assert_eq!(client.balance(), 25);
    let items = vec![
        &env,
        DeductItem {
            amount: 10,
            request_id: None,
        },
        DeductItem {
            amount: 20,
            request_id: None,
        },
        DeductItem {
            amount: 30,
            request_id: None,
        },
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.batch_deduct(&caller, &items);
    }));

    assert!(result.is_err());
    assert_eq!(client.balance(), 25);
}

#[test]
fn owner_unchanged_after_deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    fund_user(&usdc_admin, &owner, 50);
    approve_spend(&env, &usdc_client, &owner, &contract_id, 50);
    client.deposit(&owner, &50);
    client.deduct(&owner, &30, &None);
    assert_eq!(client.get_meta().owner, owner);
    client.init(&owner, &Some(100));

    env.mock_auths(&[]); // Clear mock auths so subsequent calls require explicit valid signatures

    // This should panic because neither `owner` nor `not_owner` has provided a valid mock signature.
    client.transfer_ownership(&new_owner);
}
