//! Differential invariant test harness using a reference model.

use proptest::prelude::*;
use soroban_sdk::{Address, Env};
use std::collections::HashMap;

use crate::contract::{VirtualTokenContract, VirtualTokenContractClient};
use super::reference_model::ReferenceModel;

/// Represents a simplified action that can be performed on the contract.
#[derive(Debug, Clone)]
enum Action {
    Deposit { user: Address, amount: i128 },
    Withdraw { user: Address, amount: i128 },
    BetUp { user: Address, amount: i128 },
    BetDown { user: Address, amount: i128 },
    Resolve { price_up: bool },
    Claim { user: Address },
}

/// Generate a random sequence of actions.
fn action_strategy() -> impl Strategy<Value = Action> {
    // Generate a random address.
    let addr = any::<[u8; 32]>().prop_map(|bytes| Address::from_bytes(&bytes));
    let amount = 0i128..=1_000_000i128;
    prop_oneof![
        (addr.clone(), amount.clone()).prop_map(|(u, a)| Action::Deposit { user: u, amount: a }),
        (addr.clone(), amount.clone()).prop_map(|(u, a)| Action::Withdraw { user: u, amount: a }),
        (addr.clone(), amount.clone()).prop_map(|(u, a)| Action::BetUp { user: u, amount: a }),
        (addr.clone(), amount.clone()).prop_map(|(u, a)| Action::BetDown { user: u, amount: a }),
        any::<bool>().prop_map(|up| Action::Resolve { price_up: up }),
        addr.prop_map(|u| Action::Claim { user: u }),
    ]
}

proptest! {
    #[test]
    fn differential_invariant_harness(actions in prop::collection::vec(action_strategy(), 1..30)) {
        // Setup contract environment.
        let env = Env::default();
        let contract_id = env.register(VirtualTokenContract, ());
        let client = VirtualTokenContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        env.mock_all_auths();
        client.initialize(&admin, &oracle);

        // Reference model.
        let mut model = ReferenceModel::new();

        // Helper to compare invariants after each step.
        let check = |model: &ReferenceModel| {
            let violations = model.check_invariants();
            prop_assert!(violations.is_empty(), "Invariant violations: {:#?}", violations);
        };

        // Execute actions.
        for act in actions {
            match act {
                Action::Deposit { user, amount } => {
                    client.deposit(&user, &(amount as u128));
                    model.deposit(&user, amount);
                }
                Action::Withdraw { user, amount } => {
                    let _ = client.withdraw(&user, &(amount as u128));
                    model.withdraw(&user, amount);
                }
                Action::BetUp { user, amount } => {
                    client.place_up(&user, &(amount as u128));
                    model.place_bet(&user, amount);
                }
                Action::BetDown { user, amount } => {
                    client.place_down(&user, &(amount as u128));
                    model.place_bet(&user, amount);
                }
                Action::Resolve { price_up } => {
                    client.resolve_round(&crate::types::OraclePayload {
                        price: if price_up { 2_000_0000 } else { 500_000 },
                        timestamp: env.ledger().timestamp(),
                        round_id: 0,
                        nonce: 1u64,
                    });
                    model.resolve(&HashMap::new());
                }
                Action::Claim { user } => {
                    let _ = client.claim_pending_winnings(&user);
                    model.claim(&user);
                }
            }
            check(&model);
        }
    }
}
