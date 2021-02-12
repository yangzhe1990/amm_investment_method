use super::super::amm::*;
use super::super::*;
use super::*;
use std::collections::VecDeque;

pub struct AMMCostAverage {
    amm_put_cash: Vec<f64>,
    imaginary_total_amm_returned_cash: f64,
    finished_amms: usize,
    last_amm_uninvested: f64,

    cash_invested: f64,
    coins_invested: f64,

    cash_reserve: f64,
    est_dca_cash_use_ratio: f64,
    amount_round: f64,
    imaginary_amm_ticks_to_expire: usize,
    tick: usize,

    rebalance_cash_ratio: f64,
    rebalance_step_percentage: f64,

    amms: VecDeque<AMMBuyBear>,

    buy_logs: BuyLogs,
    last_price: f64,
}

impl AMMCostAverage {
    pub fn new(
        est_dca_cash_use_ratio: f64,
        rebalance_cash_ratio: f64,
        rebalance_step_percentage: f64,
        imaginary_amm_ticks_to_expire: usize,
    ) -> Self {
        Self {
            amm_put_cash: vec![],
            imaginary_total_amm_returned_cash: 0.0,
            finished_amms: 0,
            last_amm_uninvested: 0.0,

            cash_invested: 0.0,
            coins_invested: 0.0,

            rebalance_cash_ratio,
            rebalance_step_percentage,
            est_dca_cash_use_ratio,
            imaginary_amm_ticks_to_expire,
            tick: 0,

            cash_reserve: 0.0,
            amount_round: 0.0,

            amms: Default::default(),

            buy_logs: vec![],
            last_price: 0.0,
        }
    }

    fn basic_cash_per_day(&self) -> f64 {
        self.amount_round / (DAYS_PER_ROUND as f64)
    }

    fn cash_unused(&self) -> f64 {
        // Expected to invest - already invested - (borrowed - borrow_repay).
        // self.cash_reserve = expected to invest + borrowed.
        let mut cash_unused = self.cash_reserve;
        for amm in &self.amms {
            cash_unused += amm.cash;
        }

        cash_unused
    }

    fn past_amm_cash_utilization(&self) -> (f64, f64, f64, f64) {
        if self.tick <= self.imaginary_amm_ticks_to_expire {
            return (0.0, 0.0, 0.0, std::f64::NAN);
        }

        let mut cash_put = 0.0;
        for i in 0..self.finished_amms {
            cash_put += self.amm_put_cash[i]
        }
        assert_eq!(
            self.tick - self.imaginary_amm_ticks_to_expire,
            self.finished_amms
        );

        let cash_invested = cash_put - self.imaginary_total_amm_returned_cash;
        let average_invested = cash_invested / self.finished_amms as f64;
        // cash_invested = put * (1 - CASH_RATIO) * factor
        let over_invest_factor = cash_invested / cash_put / (1.0 - self.rebalance_cash_ratio);

        (
            cash_put,
            cash_invested,
            average_invested,
            over_invest_factor,
        )
    }

    fn amm_cash_today(&mut self) -> f64 {
        let cash_day = self.basic_cash_per_day() / (1.0 - self.rebalance_cash_ratio)
            * self.est_dca_cash_use_ratio;

        self.cash_reserve += self.basic_cash_per_day() - cash_day;

        self.amm_put_cash.push(cash_day);
        cash_day
    }
}

impl CostAverageMethodTrait for AMMCostAverage {
    fn set_supply(&mut self, amount: f64) {
        self.amount_round = amount;
        self.cash_reserve = 0.0;
    }
    fn start_new_round(&mut self, ticks: usize) {
        assert_eq!(ticks, DAYS_PER_ROUND);
    }
    fn feed_price(&mut self, price: f64) {
        let basic_cash_per_day = self.basic_cash_per_day();

        for amm in self.amms.iter_mut() {
            let (expire, cash, coins) = amm.check_expiration(self.tick);
            if expire {
                self.last_amm_uninvested = cash - basic_cash_per_day;
                self.imaginary_total_amm_returned_cash += cash;
                self.finished_amms += 1;
                self.cash_reserve += cash;

                // put the money and coins back.
                amm.cash = cash;
                amm.coins = coins;
            } else {
                break;
            }
        }

        let amm_cash = self.amm_cash_today();
        self.amms.push_back(AMMBuyBear::new(
            amm_cash,
            price,
            self.tick + self.imaginary_amm_ticks_to_expire,
            self.rebalance_cash_ratio,
            self.rebalance_step_percentage,
        ));

        let mut i = self.amms.len();
        while i > 0 {
            i -= 1;

            let amm = &mut self.amms[i];
            if amm.last_price < price {
                break;
            }
            let (cash, coins) = amm.buy(price, &mut self.buy_logs);
            self.cash_invested += cash;
            self.coins_invested += coins;
        }

        self.tick += 1;
        self.last_price = price;
    }

    /// Returns (total invested cash, total invested coins)
    fn get_invest_status(&self) -> (f64, f64) {
        (self.cash_invested, self.coins_invested)
    }
}
