use super::*;

pub mod amm_cost_average;
pub mod amm_cost_average_auto;
pub mod fluctuation;

pub use self::amm_cost_average::AMMCostAverage;
pub use self::amm_cost_average_auto::AMMCostAverageAuto;

pub const DAYS_PER_ROUND: usize = 30;
pub const DOLLAR_COST_AVERAGE_SUPPLY: (f64, usize) = (2000.0, DAYS_PER_ROUND);

pub trait CostAverageMethodTrait {
    fn set_supply(&mut self, amount: f64);
    fn start_new_round(&mut self, ticks: usize);
    fn feed_price(&mut self, price: f64);
    /// Returns (total invested cash, total invested coins)
    fn get_invest_status(&self) -> (f64, f64);
}

// Returns the (additionally invested cash, final amount of coins, average coin purchase price)
pub fn dollar_cost_average(
    bull_start_index: usize,
    bear_start_index: usize,
    bear_start_coins: f64,
    lowest_date_index: usize,
    cash_invested: f64,
    begin_total_asset: f64,
    invest_method: &mut impl CostAverageMethodTrait,
    rows: &[Row],
) -> (f64, f64, f64) {
    invest_method.set_supply(DOLLAR_COST_AVERAGE_SUPPLY.0);
    let mut worst_coins_invested = bear_start_coins;
    let mut worst_cash_invested = cash_invested;

    let mut round_ticks = 0;
    let lowest_price = rows[lowest_date_index].price;
    for index in bear_start_index..bull_start_index {
        if index == lowest_date_index {
            let (cash_invested, coins_invested) = invest_method.get_invest_status();
            worst_coins_invested += coins_invested;
            worst_cash_invested += cash_invested;
        }
        if round_ticks == 0 {
            invest_method.start_new_round(DOLLAR_COST_AVERAGE_SUPPLY.1);
        }
        round_ticks += 1;
        if round_ticks == DOLLAR_COST_AVERAGE_SUPPLY.1 {
            round_ticks = 0;
        }

        invest_method.feed_price(rows[index].price);
    }
    let (bear_invest_amount, bear_invest_coins) = invest_method.get_invest_status();
    let bear_invest_average_price = bear_invest_amount / bear_invest_coins;
    println!(
        "At btc lowest price {}, total invested cash {} of {}, BTC {}. The maximum potential loss \
        if btc goes to 300: {} of {}.",
        lowest_price,
        worst_cash_invested,
        begin_total_asset + worst_cash_invested - cash_invested,
        worst_coins_invested,
        worst_cash_invested - 300.0 * worst_coins_invested,
        begin_total_asset + worst_cash_invested - cash_invested,
    );
    println!(
        "Till price {} on {} dollar average invest cash {} in bear market, BTC amount {}, average price {}",
        rows[bull_start_index].price, rows[bull_start_index].date, bear_invest_amount, bear_invest_coins, bear_invest_average_price,
    );

    (
        bear_invest_amount,
        bear_start_coins + bear_invest_coins,
        bear_invest_average_price,
    )
}

#[derive(Default)]
pub struct DollarCostAverage {
    amount_round: f64,
    total_cash: f64,
    total_coins: f64,
    tick: i32,
}

impl CostAverageMethodTrait for DollarCostAverage {
    fn set_supply(&mut self, amount: f64) {
        self.amount_round = amount;
    }
    fn start_new_round(&mut self, _ticks: usize) {
        self.tick = 0;
    }
    fn feed_price(&mut self, price: f64) {
        if self.tick == 0 {
            self.total_cash += self.amount_round;
            self.total_coins += self.amount_round / price;
            self.tick = -1;
        }
    }

    /// Returns (total invested cash, total invested coins)
    fn get_invest_status(&self) -> (f64, f64) {
        (self.total_cash, self.total_coins)
    }
}

#[derive(Default)]
pub struct DailyDollarCostAverage {
    amount_round: f64,
    total_cash: f64,
    total_coins: f64,
    ticks: f64,
}

impl CostAverageMethodTrait for DailyDollarCostAverage {
    fn set_supply(&mut self, amount: f64) {
        self.amount_round = amount;
    }
    fn start_new_round(&mut self, ticks: usize) {
        self.ticks = ticks as f64;
    }
    fn feed_price(&mut self, price: f64) {
        self.total_cash += self.amount_round / self.ticks;
        self.total_coins += self.amount_round / self.ticks / price;
    }

    /// Returns (total invested cash, total invested coins)
    fn get_invest_status(&self) -> (f64, f64) {
        (self.total_cash, self.total_coins)
    }
}
