use super::*;
use soroban_sdk::testutils::Address as _;

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, Some(1000));
    assert_eq!(client.balance(), 1000);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register_contract(None, CalloraVault {});
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, Some(100));
    client.deposit(&200);
    assert_eq!(client.balance(), 300);
    client.deduct(&50);
    assert_eq!(client.balance(), 250);
}
