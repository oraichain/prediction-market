use crate::market::*;

mod constants;
mod errors;
mod instructions;
mod lmsr;
mod market;
mod storage_impl;
mod token_receiver;
mod views;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    markets: Vector<Market>,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            markets: Vector::new(b"near-prediction".to_vec()),
        }
    }
}

type MarketId = u64;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn create_market(&mut self, args: CreateMarketArgs) -> MarketId {
        let market_id: MarketId = self.markets.len();
        let market = Market::new(market_id, args);
        self.markets.push(&market);
        market_id
    }

    pub fn get_markets(&self) -> u64 {
        self.markets.len()
    }

    fn get_market(&self, market_id: u64) -> Market {
        self.markets.get(market_id).unwrap()
    }

    pub fn open_market(&mut self, market_id: MarketId) {
        let mut market = self.get_market(market_id);
        assert_eq!(market.operator, env::signer_account_id());
        market.open();
        self.markets.replace(market.id, &market);
    }

    pub fn pause_market(&mut self, market_id: MarketId) {
        let mut market = self.get_market(market_id);
        market.pause();
    }

    pub fn resolve_market(&mut self, market_id: MarketId, payouts: Vec<u128>) {
        let mut market = self.get_market(market_id);
        assert!(market.stage == Stage::Paused || market.stage == Stage::Open);
        assert_eq!(market.outcomes.len(), payouts.len() as u64);

        // TODO(cqsd): wait for merge
        // let expected_payout_vec_sum: u128 = 10u128.pow(market.collateral_decimals);
        // use this as the match guard

        let outcome_id = payouts
            .iter()
            .enumerate()
            .max_by(|(_, value0), (_, value1)| value0.cmp(value1))
            .map(|(idx, _)| idx)
            .unwrap() as u32;
        match payouts.iter().sum::<u128>() {
            s if (s == market.collateral_decimals as u128) => {
                // usual case, resolve the market
                market.payouts = Some(payouts);
                market.stage = Stage::Finalized(Finalization::Resolved { outcome_id });
            }
            0 => {
                // no payouts --> the outcomes were invalid, put market in refund mode
                market.payouts = None;
                market.stage = Stage::Finalized(Finalization::Invalid);
                // TODO(cqsd): need to handle the refund state in redeem?
            }
            _ => env::panic(b"Invalid payout vector"),
        };

        self.markets.replace(market_id, &market);
    }

    pub fn buy(
        &mut self,
        sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        ix: instructions::Buy,
    ) -> PromiseOrValue<U128> {
        log!(
            "buy: sender_id: {} token_id: {} amount: {}",
            sender_id,
            token_id,
            amount
        );
        let mut market = self.get_market(ix.market_id.into());
        assert_eq!(market.collateral_token, *token_id);

        let ret = market.internal_buy(&sender_id, amount, ix.num_shares as u128, ix.outcome_id);
        self.markets.replace(market.id, &market);

        ret
    }

    pub fn sell(
        &mut self,
        token_id: &AccountId,
        amount: Balance,
        market_id: u64,
        outcome_id: u32,
        num_shares: u64,
    ) {
        let mut market = self.get_market(market_id);
        assert_eq!(market.collateral_token, *token_id);
        let seller_id = env::signer_account_id();

        market.internal_sell(&seller_id, amount, num_shares as u128, outcome_id);
        self.markets.replace(market.id, &market);
    }

    pub fn deposit(
        &mut self,
        _sender_id: &AccountId,
        token_id: &AccountId,
        amount: Balance,
        ix: instructions::InitialDeposit,
    ) -> PromiseOrValue<U128> {
        let mut market = self.get_market(ix.market_id.into());
        assert_eq!(market.collateral_token, *token_id);
        market.deposit_collateral(amount);

        self.markets.replace(market.id, &market);

        PromiseOrValue::Value(U128(0))
    }

    pub fn withdraw_fees(&mut self, market_id: MarketId) -> Promise {
        let mut market = self.get_market(market_id.into());
        let ret = market.withdraw_fees();

        self.markets.replace(market.id, &market);

        ret
    }
}
