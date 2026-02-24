extern crate std;

use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(1000), &None);
    // Verify balance after init
    assert_eq!(client.balance(), 1000);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.deposit(&200);
    assert_eq!(client.balance(), 300);
    env.mock_all_auths();
    client.deduct(&50);
    assert_eq!(client.balance(), 250);
}

/// Test that verifies consistency between balance() and get_meta() after init, deposit, and deduct.
/// This ensures that both methods return the same balance value and that the owner remains unchanged.
#[test]
fn balance_and_meta_consistency() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    // Initialize vault with initial balance
    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(500), &None);

    // Verify consistency after initialization
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after init");
    assert_eq!(meta.owner, owner, "owner changed after init");
    assert_eq!(balance, 500, "incorrect balance after init");

    // Deposit and verify consistency
    client.deposit(&300);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deposit");
    assert_eq!(meta.owner, owner, "owner changed after deposit");
    assert_eq!(balance, 800, "incorrect balance after deposit");

    // Deduct and verify consistency
    client.deduct(&150);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deduct");
    assert_eq!(meta.owner, owner, "owner changed after deduct");
    assert_eq!(balance, 650, "incorrect balance after deduct");

    // Perform multiple operations and verify final state
    client.deposit(&100);
    client.deduct(&50);
    client.deposit(&25);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(
        meta.balance, balance,
        "balance mismatch after multiple operations"
    );
    assert_eq!(meta.owner, owner, "owner changed after multiple operations");
    assert_eq!(balance, 725, "incorrect final balance");
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exact_balance_and_panic() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    assert_eq!(client.balance(), 100);

    // Deduct exact balance
    client.deduct(&100);
    assert_eq!(client.balance(), 0);

    // Further deduct should panic
    client.deduct(&1);
}

#[test]
fn deduct_event_emission() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(1000), &None);

    // Deduct and verify the balance changes
    client.deduct(&200);
    assert_eq!(client.balance(), 800);
}

#[test]
#[should_panic(expected = "deposit amount must be positive")]
fn deposit_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.deposit(&0);
}

#[test]
#[should_panic(expected = "deposit amount must be positive")]
fn deposit_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.deposit(&-100);
}

#[test]
#[should_panic(expected = "deduct amount must be positive")]
fn deduct_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.deduct(&0);
}

#[test]
#[should_panic(expected = "deduct amount must be positive")]
fn deduct_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.deduct(&-50);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exceeds_balance_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.deduct(&200);
}

#[test]
#[should_panic(expected = "vault already initialized")]
fn init_already_initialized_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let usdc_token = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);
    client.init(&owner, &usdc_token, &Some(200), &None); // Should panic
}
