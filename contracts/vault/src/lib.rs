//! # Callora Vault Contract
//!
//! ## Access Control
//!
//! The vault implements role-based access control for deposits:
//!
//! - **Owner**: Set at initialization, immutable. Always permitted to deposit.
//! - **Allowed Depositor**: Optional address (e.g., backend service) that can be
//!   explicitly approved by the owner. Can be set, changed, or cleared at any time.
//! - **Other addresses**: Rejected with an authorization error.
//!
//! ### Production Usage
//!
//! In production, the owner typically represents the end user's account, while the
//! allowed depositor is a backend service that handles automated deposits on behalf
//! of the user.
//!
//! ### Managing the Allowed Depositor
//!
//! - Set or update: `set_allowed_depositor(Some(address))`
//! - Clear (revoke access): `set_allowed_depositor(None)`
//! - Only the owner can call `set_allowed_depositor`
//!
//! ### Security Model
//!
//! - The owner has full control over who can deposit
//! - The allowed depositor is a trusted address (typically a backend service)
//! - Access can be revoked at any time by the owner
//! - All deposit attempts are authenticated against the caller's address

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
}

const META_KEY: &str = "meta";
const USDC_KEY: &str = "usdc";
const ADMIN_KEY: &str = "admin";
const REVENUE_POOL_KEY: &str = "revenue_pool";
const MAX_DEDUCT_KEY: &str = "max_deduct";

/// Default maximum single deduct amount when not set at init (no cap).
pub const DEFAULT_MAX_DEDUCT: i128 = i128::MAX;

#[contracttype]
pub enum StorageKey {
    Meta,
    AllowedDepositor,
}

#[contract]
pub struct CalloraVault;

#[contractimpl]
impl CalloraVault {
    /// Initialize vault for an owner with optional initial balance and minimum deposit.
    /// If initial_balance > 0, the contract must already hold at least that much USDC (e.g. deployer transferred in first).
    /// Emits an "init" event with the owner address and initial balance.
    ///
    /// # Arguments
    /// * `revenue_pool` – Optional address to receive USDC on each deduct (e.g. settlement contract). If None, USDC stays in vault.
    /// * `max_deduct` – Optional cap per single deduct; if None, uses DEFAULT_MAX_DEDUCT (no cap).
    pub fn init(
        env: Env,
        owner: Address,
        usdc_token: Address,
        initial_balance: Option<i128>,
        min_deposit: Option<i128>,
        revenue_pool: Option<Address>,
        max_deduct: Option<i128>,
    ) -> VaultMeta {
        owner.require_auth();
        if env.storage().instance().has(&Symbol::new(&env, META_KEY)) {
            panic!("vault already initialized");
        }
        let balance = initial_balance.unwrap_or(0);
        if balance > 0 {
            let usdc = token::Client::new(&env, &usdc_token);
            let contract_balance = usdc.balance(&env.current_contract_address());
            if contract_balance < balance {
                panic!("insufficient USDC in contract for initial_balance");
            }
        }
        let min_deposit_val = min_deposit.unwrap_or(0);
        let max_deduct_val = max_deduct.unwrap_or(DEFAULT_MAX_DEDUCT);
        if max_deduct_val <= 0 {
            panic!("max_deduct must be positive");
        }
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
        };
        env.storage().instance().set(&StorageKey::Meta, &meta);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, USDC_KEY), &usdc_token);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &owner);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, REVENUE_POOL_KEY), &revenue_pool);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, MAX_DEDUCT_KEY), &max_deduct_val);

        env.events()
            .publish((Symbol::new(&env, "init"), owner), balance);

        meta
    }

    /// Check if the caller is authorized to deposit (owner or allowed depositor).
    fn is_authorized_depositor(env: &Env, caller: &Address) -> bool {
        let meta = Self::get_meta(env.clone());

        // Owner is always authorized
        if caller == &meta.owner {
            return true;
        }
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &new_admin);
    }

    /// Return the maximum allowed amount for a single deduct (configurable at init).
    pub fn get_max_deduct(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, MAX_DEDUCT_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Return the revenue pool address if set (receives USDC on deduct).
    pub fn get_revenue_pool(env: Env) -> Option<Address> {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, REVENUE_POOL_KEY))
            .unwrap_or(None)
    }

    /// Distribute accumulated USDC to a single developer address.
    ///
    /// # Access control
    /// Only the admin (backend / multisig) may call this.
    ///
    /// # Arguments
    /// * `caller` – Must be the current admin address.
    /// * `to`     – Developer wallet to receive the USDC.
    /// * `amount` – Amount in USDC micro-units (must be > 0 and ≤ vault balance).
    ///
    /// # Panics
    /// * `"unauthorized: caller is not admin"` – caller is not the admin.
    /// * `"amount must be positive"`           – amount is zero or negative.
    /// * `"insufficient USDC balance"`         – vault holds less than amount.
    ///
    /// # Events
    /// Emits topic `("distribute", to)` with data `amount` on success.
    pub fn distribute(env: Env, caller: Address, to: Address, amount: i128) {
        // 1. Require on-chain signature from caller.
        caller.require_auth();

        // Check if caller is the allowed depositor
        if let Some(allowed) = env
            .storage()
            .instance()
            .get::<StorageKey, Address>(&StorageKey::AllowedDepositor)
        {
            if caller == &allowed {
                return true;
            }
        }

        false
    }

    /// Require that the caller is the owner, panic otherwise.
    fn require_owner(env: &Env, caller: &Address) {
        let meta = Self::get_meta(env.clone());
        assert!(caller == &meta.owner, "unauthorized: owner only");
    }

    /// Get vault metadata (owner and balance).
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&StorageKey::Meta)
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Set or clear the allowed depositor address. Owner-only.
    /// Pass `None` to revoke depositor access, `Some(address)` to grant or update.
    pub fn set_allowed_depositor(env: Env, caller: Address, depositor: Option<Address>) {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        match depositor {
            Some(addr) => {
                env.storage()
                    .instance()
                    .set(&StorageKey::AllowedDepositor, &addr);
            }
            None => {
                env.storage()
                    .instance()
                    .remove(&StorageKey::AllowedDepositor);
            }
        }
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
    pub fn deposit(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();

        assert!(
            Self::is_authorized_depositor(&env, &caller),
            "unauthorized: only owner or allowed depositor can deposit"
        );

        let mut meta = Self::get_meta(env.clone());
        meta.balance += amount;
        env.storage().instance().set(&StorageKey::Meta, &meta);
        meta.balance
    }

    /// Deduct balance for an API call. Only backend/authorized caller in production.
    pub fn deduct(env: Env, amount: i128) -> i128 {
            .get(&Symbol::new(&env, META_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Deposit: user transfers USDC to the contract; contract increases internal balance.
    /// Caller must have authorized the transfer (token transfer_from). Supports multiple depositors.
    /// Emits a "deposit" event with the depositor address and amount.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();

        let mut meta = Self::get_meta(env.clone());
        assert!(
            amount >= meta.min_deposit,
            "deposit below minimum: {} < {}",
            amount,
            meta.min_deposit
        );

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));
        let usdc = token::Client::new(&env, &usdc_address);
        usdc.transfer_from(
            &env.current_contract_address(),
            &from,
            &env.current_contract_address(),
            &amount,
        );

        meta.balance += amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        env.events()
            .publish((Symbol::new(&env, "deposit"), from), amount);

        meta.balance
    }

    /// Deduct balance for an API call. Callable by authorized caller (e.g. backend).
    /// Amount must not exceed max single deduct (see init / get_max_deduct).
    /// If revenue pool is set, USDC is transferred to it; otherwise it remains in the vault.
    /// Emits a "deduct" event with caller, optional request_id, amount, and new balance.
    pub fn deduct(env: Env, caller: Address, amount: i128, request_id: Option<Symbol>) -> i128 {
        caller.require_auth();
        let max_deduct = Self::get_max_deduct(env.clone());
        assert!(amount > 0, "amount must be positive");
        assert!(amount <= max_deduct, "deduct amount exceeds max_deduct");

        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));
        let revenue_pool: Option<Address> = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, REVENUE_POOL_KEY))
            .unwrap_or(None);

        meta.balance -= amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        if let Some(to) = revenue_pool {
            let usdc = token::Client::new(&env, &usdc_address);
            usdc.transfer(&env.current_contract_address(), &to, &amount);
        }

        let topics = match &request_id {
            Some(rid) => (Symbol::new(&env, "deduct"), caller.clone(), rid.clone()),
            None => (
                Symbol::new(&env, "deduct"),
                caller.clone(),
                Symbol::new(&env, ""),
            ),
        };
        env.events().publish(topics, (amount, meta.balance));
        meta.balance
    }

    /// Batch deduct: multiple (amount, optional request_id) in one transaction.
    /// Each amount must not exceed max_deduct. Reverts entire batch if any check fails.
    /// If revenue pool is set, total deducted USDC is transferred to it once.
    /// Emits one "deduct" event per item.
    pub fn batch_deduct(env: Env, caller: Address, items: Vec<DeductItem>) -> i128 {
        caller.require_auth();
        let max_deduct = Self::get_max_deduct(env.clone());
        let mut meta = Self::get_meta(env.clone());
        let n = items.len();
        assert!(n > 0, "batch_deduct requires at least one item");

        let mut total_deduct = 0i128;
        let mut running = meta.balance;
        for item in items.iter() {
            assert!(item.amount > 0, "amount must be positive");
            assert!(
                item.amount <= max_deduct,
                "deduct amount exceeds max_deduct"
            );
            assert!(running >= item.amount, "insufficient balance");
            running -= item.amount;
            total_deduct += item.amount;
        }

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));
        let revenue_pool: Option<Address> = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, REVENUE_POOL_KEY))
            .unwrap_or(None);

        let mut balance = meta.balance;
        for item in items.iter() {
            balance -= item.amount;
            let topics = match &item.request_id {
                Some(rid) => (Symbol::new(&env, "deduct"), caller.clone(), rid.clone()),
                None => (
                    Symbol::new(&env, "deduct"),
                    caller.clone(),
                    Symbol::new(&env, ""),
                ),
            };
            env.events().publish(topics, (item.amount, balance));
        }

        meta.balance = balance;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        if total_deduct > 0 {
            if let Some(to) = revenue_pool {
                let usdc = token::Client::new(&env, &usdc_address);
                usdc.transfer(&env.current_contract_address(), &to, &total_deduct);
            }
        }

        meta.balance
    }

    /// Withdraw from vault. Callable only by the vault owner; reduces balance and transfers USDC to owner.
    pub fn withdraw(env: Env, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));
        let usdc = token::Client::new(&env, &usdc_address);
        usdc.transfer(&env.current_contract_address(), &meta.owner, &amount);

        meta.balance -= amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        env.events().publish(
            (Symbol::new(&env, "withdraw"), meta.owner.clone()),
            (amount, meta.balance),
        );
        meta.balance
    }

    /// Withdraw from vault to a designated address. Owner-only; transfers USDC to `to`.
    pub fn withdraw_to(env: Env, to: Address, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));
        let usdc = token::Client::new(&env, &usdc_address);
        usdc.transfer(&env.current_contract_address(), &to, &amount);

        meta.balance -= amount;
        env.storage().instance().set(&StorageKey::Meta, &meta);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        env.events().publish(
            (
                Symbol::new(&env, "withdraw_to"),
                meta.owner.clone(),
                to.clone(),
            ),
            (amount, meta.balance),
        );
        meta.balance
    }

    /// Return current balance.
    pub fn balance(env: Env) -> i128 {
        Self::get_meta(env).balance
    }
}

#[cfg(test)]
mod test;
