use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(1000));
    assert_eq!(client.balance(), 1000);
}

#[test]
fn init_default_zero_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &None);
    assert_eq!(client.balance(), 0);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deposit(&200);
    assert_eq!(client.balance(), 300);
    client.deduct(&50);
    assert_eq!(client.balance(), 250);
}

#[test]
fn deduct_exact_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(500));
    client.deduct(&500);
    assert_eq!(client.balance(), 0);
}

// ───────────────── Overflow / underflow tests ─────────────────

#[test]
#[should_panic(expected = "deposit overflow")]
fn deposit_overflow_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(i128::MAX));
    client.deposit(&1); // overflow
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_underflow_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(50));
    client.deduct(&100); // insufficient balance
}

// ───────────────── Input validation tests ─────────────────────

#[test]
#[should_panic(expected = "amount must be positive")]
fn deposit_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deposit(&-100);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deduct(&-50);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deposit_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deposit(&0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deduct(&0);
}

#[test]
#[should_panic(expected = "initial balance must be non-negative")]
fn init_negative_balance_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(-500));
}
