use super::super::amm::*;
use super::super::*;
use super::*;
use std::collections::VecDeque;

pub struct AMMCostAverageAuto {
    amm_put_cash: Vec<f64>,
    total_amm_returned_cash: f64,
    finished_amms: usize,
    last_amm_uninvested: f64,

    cash_invested: f64,
    coins_invested: f64,

    cash_reserve: f64,
    est_dca_cash_use_ratio: f64,
    amount_round: f64,
    amm_ticks_to_expire: usize,
    tick: usize,

    rebalance_cash_ratio: f64,
    rebalance_step_percentage: f64,

    amms: VecDeque<AMMBuyBear>,

    buy_logs: BuyLogs,
    last_price: f64,

    // auto adjust day cash
    // the past uninvested amount should be used in (1/...) days.
    past_uninvested_reinvest_daily_percentage: f64,
}

impl AMMCostAverageAuto {
    pub fn new(
        est_dca_cash_use_ratio: f64,
        rebalance_cash_ratio: f64,
        rebalance_step_percentage: f64,
        amm_ticks_to_expire: usize,
        past_uninvested_reinvest_daily_percentage: f64,
    ) -> Self {
        Self {
            amm_put_cash: vec![],
            total_amm_returned_cash: 0.0,
            finished_amms: 0,
            last_amm_uninvested: 0.0,

            cash_invested: 0.0,
            coins_invested: 0.0,

            rebalance_cash_ratio,
            rebalance_step_percentage,
            est_dca_cash_use_ratio,
            amm_ticks_to_expire,
            tick: 0,

            cash_reserve: 0.0,
            amount_round: 0.0,

            amms: Default::default(),

            buy_logs: vec![],
            last_price: 0.0,
            past_uninvested_reinvest_daily_percentage,
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
        if self.tick <= self.amm_ticks_to_expire {
            return (0.0, 0.0, 0.0, std::f64::NAN);
        }

        let mut cash_put = 0.0;
        for i in 0..self.finished_amms {
            cash_put += self.amm_put_cash[i]
        }
        assert!(
            (self.tick - self.amm_ticks_to_expire) == self.finished_amms
                || (self.tick - self.amm_ticks_to_expire) == self.finished_amms - 1
        );

        let cash_invested = cash_put - self.total_amm_returned_cash;
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

    fn log_cash_unused(&self) {
        println!(
            "cash_reserve {} cash_unused {} coin cap {}",
            self.cash_reserve,
            self.cash_unused(),
            self.coins_invested * self.last_price,
        );
        let (cash_put, finished_amm_cash_invested, average_invested, over_invest_ratio) =
            self.past_amm_cash_utilization();
        println!(
            "finished_amm(cash_put: {}, cash_invested: {}), amm over_invest_ratio {}, average {}, expected {}",
            cash_put,
            finished_amm_cash_invested,
            over_invest_ratio,
            average_invested,
            self.basic_cash_per_day(),
        )
    }

    fn amm_cash_today(&mut self) -> f64 {
        let mut cash_day = self.basic_cash_per_day() / (1.0 - self.rebalance_cash_ratio)
            * self.est_dca_cash_use_ratio;

        if self.tick > self.amm_ticks_to_expire {
            // there are two parts: finished amms, unfinished amms. we want to know if
            // we are investing fewer or more than expected and adjust accordingly.

            // Check utilization for finished amms.
            assert_eq!(self.finished_amms, self.tick - self.amm_ticks_to_expire + 1);
            let (_cash_put, finished_amm_cash_invested, average_invested, over_invest_ratio) =
                self.past_amm_cash_utilization();
            let expected_spending =
                self.basic_cash_per_day() * (self.tick - self.amm_ticks_to_expire + 1) as f64;
            let daily_diff = self.basic_cash_per_day() - average_invested;
            let difference = expected_spending - finished_amm_cash_invested;

            // If we invest more, there are two factors: 1. est_dca_cash_use_ratio is too high,
            // the real one is 1 / over_invest_ratio; 2. the market is too strong, however in a
            // bear market we shouldn't invest too much if the price is kept strong, so we try not
            // to allocate too much budget for the new daily AMM. If the price falls immediately,
            // the existing AMM will buy the dip.
            let adjust_1 = difference * self.past_uninvested_reinvest_daily_percentage;
            let adjust_2 = self.basic_cash_per_day()
                * (1.0 / (over_invest_ratio * self.est_dca_cash_use_ratio) - 1.0);
            let adjust = adjust_1 + adjust_2;

            cash_day += adjust / (1.0 - self.rebalance_cash_ratio) * self.est_dca_cash_use_ratio;

            println!(
                "est_dca_cash_use_ratio {}, expected {}, total past uninvested {}, price {}, \
                 basic_cash_per_day {}, daily average invested {}, daily diff {}, \
                 adjustment {} = {} + {}, cash_day {}",
                self.est_dca_cash_use_ratio,
                1.0 / over_invest_ratio,
                difference,
                self.last_price,
                self.basic_cash_per_day(),
                average_invested,
                daily_diff,
                adjust,
                adjust_1,
                adjust_2,
                cash_day,
            );
            if cash_day < 0.0 {
                cash_day = 0.0;
            }

            // Second part, check the active amms.
        };

        self.cash_reserve += self.basic_cash_per_day() - cash_day;

        self.amm_put_cash.push(cash_day);
        cash_day
    }
}

impl CostAverageMethodTrait for AMMCostAverageAuto {
    fn set_supply(&mut self, amount: f64) {
        self.amount_round = amount;
        self.cash_reserve = 0.0;
    }
    fn start_new_round(&mut self, ticks: usize) {
        self.log_cash_unused();
        assert_eq!(ticks, DAYS_PER_ROUND);
    }
    fn feed_price(&mut self, price: f64) {
        loop {
            if let Some(amm) = self.amms.front_mut() {
                let (expire, cash, _coins) = amm.check_expiration(self.tick);
                if expire {
                    self.last_amm_uninvested = cash - self.basic_cash_per_day();
                    self.total_amm_returned_cash += cash;
                    self.finished_amms += 1;
                    self.cash_reserve += cash;
                } else {
                    break;
                }
            } else {
                break;
            }
            self.amms.pop_front();
        }

        let amm_cash = self.amm_cash_today();
        self.amms.push_back(AMMBuyBear::new(
            amm_cash,
            price,
            self.tick + self.amm_ticks_to_expire,
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
        self.log_cash_unused();
        (self.cash_invested, self.coins_invested)
    }
}
