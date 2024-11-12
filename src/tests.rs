use crate::instructions::{Buy, InitialDeposit, Instruction};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use std::convert::TryFrom;

use super::*;

const CURRENT_ACCOUNT_ID: &'static str = "contract.testnet";
const SIGNER_ACCOUNT_ID: &'static str = "alice.testnet";
const PREDECESSOR_ACCOUNT_ID: &'static str = "alice.testnet";
const ONE_HOUR_NS: u64 = 60 * 60 * 1_000_000_000;

fn get_context(input: Vec<u8>, is_view: bool) -> VMContext {
    VMContext {
        current_account_id: CURRENT_ACCOUNT_ID.to_string(),
        signer_account_id: SIGNER_ACCOUNT_ID.to_string(),
        signer_account_pk: vec![0, 1, 2],
        predecessor_account_id: PREDECESSOR_ACCOUNT_ID.to_string(),
        input,
        block_index: 0,
        block_timestamp: 0,
        account_balance: 0,
        account_locked_balance: 0,
        storage_usage: 0,
        attached_deposit: 0,
        prepaid_gas: 10u64.pow(18),
        random_seed: vec![0, 1, 2],
        is_view,
        output_data_receivers: vec![],
        epoch_height: 19,
    }
}

fn create_test_market(num_outcomes: u32) -> CreateMarketArgs {
    CreateMarketArgs {
        title: "Will Donald Trump win the 2024 US Election?".into(),
        description: "This question will be settled based on Associated Press (AP) election calls."
            .into(),
        collateral_token: "test.near".into(),
        collateral_decimals: 9,
        trade_fee_bps: 1,
        resolution_time: env::block_timestamp() + ONE_HOUR_NS,
        end_time: env::block_timestamp() + ONE_HOUR_NS,
        fee_owner: None,
        oracle: None,
        operator: None,
        outcomes: (0..num_outcomes)
            .map(|i| Outcome {
                id: i,
                short_name: "Test".into(),
                long_name: "Test".into(),
            })
            .collect(),
        liquidity: Some(50.0),
    }
}

#[test]
fn add_market() {
    let context = get_context(vec![], false);
    testing_env!(context);
    let mut contract = Contract {
        markets: Vector::new(b"mk".to_vec()),
    };
    assert_eq!(0, contract.get_markets());
    let args = create_test_market(2);
    contract.create_market(args);
    assert_eq!(1, contract.get_markets());
}

#[test]
fn buy_shares() {
    let context = get_context(vec![], false);
    testing_env!(context);
    let mut contract = Contract {
        markets: Vector::new(b"mk".to_vec()),
    };
    let args = create_test_market(2);
    let market_id = contract.create_market(args);
    let account_id: AccountId = SIGNER_ACCOUNT_ID.into();
    let mut market = contract.markets.get(market_id).unwrap();
    market.deposit_collateral(100_000_000_000);
    market.open();
    assert_eq!(None, market.outcome_balance(&account_id, 0));
    assert_eq!(None, market.outcome_balance(&account_id, 1));
    market.credit(&account_id, 0, 5);
    assert_eq!(Some(5), market.outcome_balance(&account_id, 0));
    assert_eq!(Some(0), market.outcome_balance(&account_id, 1));
}

#[test]
fn buy_price_increase() {
    let context = get_context(vec![], false);
    testing_env!(context);
    let mut contract = Contract {
        markets: Vector::new(b"mk".to_vec()),
    };
    let args = create_test_market(2);
    let market_id = contract.create_market(args);
    let market = contract.markets.get(market_id).unwrap();
    let buy_price = market.calc_price_without_fee(0, 10, OrderDirection::Buy);
    assert!(buy_price > 5_200_000_000);
}

#[test]
fn sell_price_decrease() {
    let context = get_context(vec![], false);
    testing_env!(context);
    let mut contract = Contract {
        markets: Vector::new(b"mk".to_vec()),
    };
    let args = create_test_market(2);
    let market_id = contract.create_market(args);
    let mut market = contract.markets.get(market_id).unwrap();
    let account_id: AccountId = "test_account".into();
    market.deposit_collateral(100_000_000_000);
    market.open();
    market.credit(&account_id, 1, 100);
    // Selling more shares will reduce the average price
    let max_sell_price = market.calc_sell_price(1, 10) / 10;
    let mid_sell_price = market.calc_sell_price(1, 50) / 50;
    let min_sell_price = market.calc_sell_price(1, 100) / 100;
    assert!(min_sell_price < mid_sell_price);
    assert!(mid_sell_price < max_sell_price);
}

#[test]
fn test_ft_on_transfer_buy() {
    let context = get_context(vec![], false);
    testing_env!(context);
    let mut contract = Contract {
        markets: Vector::new(b"mk".to_vec()),
    };
    let args = create_test_market(2);
    let market_id = contract.create_market(args);
    let account_id: AccountId = "alice.testnet".into();
    let token_id: AccountId = "test.near".into();
    contract.deposit(
        &account_id,
        &token_id,
        100 * 1_000_000_000,
        InitialDeposit { market_id },
    );
    contract.open_market(market_id);
    contract.buy(
        &account_id,
        &token_id,
        3 * 1_000_000_000,
        Buy {
            market_id: market_id,
            outcome_id: 0,
            num_shares: 5,
        },
    );
    let balances = contract.get_user_balances(&account_id);
    assert!(balances.len() > 0);
    assert_eq!(balances[0].shares, 5);
    assert_eq!(balances[0].market_id, market_id);
    assert_eq!(balances[0].outcome_id, 0);

    contract.sell(&token_id, 1, market_id, 0, 1);
    let new_balances = contract.get_user_balances(&account_id);
    assert_eq!(new_balances[0].shares, 4);
}
