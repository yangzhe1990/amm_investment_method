// Copyright 2021 Conflux Foundation. All rights reserved.
// Conflux is free software and distributed under GNU General Public License.
// See http://www.gnu.org/licenses/
//
// Modification based on https://github.com/hlb8122/rust-cfx-addr in MIT License.
// A copy of the original license is included in LICENSE.rust-cfx-addr.

use std::collections::VecDeque;

#[derive(Deserialize, Debug)]
struct Row {
    date: String,
    price: f64,
}

type BuyLogs = Vec<(f64, f64)>;

// rebalance_percent_steps: 1%: rebalance for each 1% change of the price.
// finish_price: only sell when start_price is lower than finish price.
// sell_log: a list of (price, amount) to sell.
fn exit_insane_bull(
    cash_ratio: f64,
    start_cash: &mut f64,
    start_coins: &mut f64,
    rebalance_percent_steps: f64,
    start_price: &mut f64,
    finish_price: f64,
    sell_log: &mut BuyLogs,
) {
    if finish_price <= *start_price {
        return;
    }

    let mut price: f64 = *start_price;
    let mut cash: f64 = *start_cash as f64;
    let mut coins: f64 = *start_coins as f64;

    while price < finish_price {
        let total = cash + coins * price;
        let take_out = total * cash_ratio - cash;

        if take_out > 0.0 {
            cash += take_out;
            let coins_to_sell = take_out / price;
            coins -= coins_to_sell;

            sell_log.push((price, -coins_to_sell));
        }

        price += price * rebalance_percent_steps;
    }

    *start_cash = cash;
    *start_price = price;
    *start_coins = coins;
}

trait CostAverageMethodTrait {
    fn set_supply(&mut self, amount: f64);
    fn start_new_round(&mut self, ticks: usize);
    fn feed_price(&mut self, price: f64);
    /// Returns (total invested cash, total invested coins)
    fn get_invest_status(&self) -> (f64, f64);
}

const DAYS_PER_ROUND: usize = 30;
const DOLLAR_COST_AVERAGE_SUPPLY: (f64, usize) = (2000.0, DAYS_PER_ROUND);

// Returns the (additionally invested cash, final amount of coins, average coin purchase price)
fn dollar_cost_average(
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
struct DollarCostAverage {
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
struct DailyDollarCostAverage {
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

struct AMMBuyBear {
    last_price: f64,
    tick_to_expire: usize,
    cash: f64,
    coins: f64,

    rebalance_cash_ratio: f64,
    rebalance_step_percentage: f64,
}

impl AMMBuyBear {
    fn new(
        cash: f64,
        price: f64,
        tick_to_expire: usize,
        rebalance_cash_ratio: f64,
        rebalance_step_percentage: f64,
    ) -> Self {
        Self {
            last_price: price,
            cash,
            tick_to_expire,
            coins: 0.0,
            rebalance_cash_ratio,
            rebalance_step_percentage,
        }
    }

    fn check_expiration(&mut self, tick: usize) -> (bool, f64, f64) {
        let take_out;
        if tick >= self.tick_to_expire {
            take_out = (true, self.cash, self.coins);
            self.cash = 0.0;
            self.coins = 0.0;
        } else {
            take_out = (false, 0.0, 0.0);
        }

        take_out
    }

    /// Only buy when price goes down.
    /// Returns: (cash spent, coins bought).
    fn buy(&mut self, new_price: f64, buy_logs: &mut BuyLogs) -> (f64, f64) {
        let mut price = self.last_price;
        if new_price > price {
            return (0.0, 0.0);
        }
        let mut cash_invested = 0.0;
        let mut coins_invested = 0.0;
        // price goes up, buy/sell some coins.
        // price goes down, buy some coins.
        while price >= new_price {
            let total = self.cash + self.coins * price;
            let buy = self.cash - total * self.rebalance_cash_ratio;

            if buy > 0.0 {
                let coins_to_buy = buy / price;
                self.cash -= buy;
                self.coins += coins_to_buy;
                cash_invested += buy;
                coins_invested += coins_to_buy;

                buy_logs.push((price, coins_to_buy));
            }

            price -= price * self.rebalance_step_percentage;
        }
        self.last_price = price;

        (cash_invested, coins_invested)
    }
}

struct AMMCostAverage {
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
}

impl AMMCostAverage {
    fn new(
        est_dca_cash_use_ratio: f64,
        rebalance_cash_ratio: f64,
        rebalance_step_percentage: f64,
        amm_ticks_to_expire: usize,
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
        for i in 0..self.tick - self.amm_ticks_to_expire {
            cash_put += self.amm_put_cash[i]
        }
        assert_eq!(self.tick - self.amm_ticks_to_expire, self.finished_amms);

        let cash_invested = cash_put - self.total_amm_returned_cash;
        let average_invested = cash_invested / (self.tick - self.amm_ticks_to_expire) as f64;
        // cash_invested = put * (1 - CASH_RATIO) * factor
        let over_invest_factor = cash_invested / cash_put / (1.0 - self.rebalance_cash_ratio);

        (cash_put, cash_invested, average_invested, over_invest_factor)
    }

    fn log_cash_unused(&self) {
        return;
        println!(
            "cash_reserve {} cash_unused {} coin cap {}",
            self.cash_reserve,
            self.cash_unused(),
            self.coins_invested * self.last_price,
        );
        let (cash_put, finished_amm_cash_invested, average_invested, over_invest_ratio) = self.past_amm_cash_utilization();
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
            // FIXME:
            /*
            let mut cash_adjust = self.cash_unused() / (self.tick - self.amm_ticks_to_expire) as f64
                * self.est_dca_cash_use_ratio / (1.0 - self.rebalance_cash_ratio);
            cash_day += cash_adjust;

             */
        };

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

#[test]
fn test_tsv_file_read() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::current_dir()?;
    println!("The current directory is {}", path.display());

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .double_quote(false)
        .escape(Some(b'\\'))
        .flexible(true)
        .comment(Some(b'#'))
        .from_path("./src/bitcoin_price_hist_since_first_10000.tsv")?;
    let mut rows = Vec::<Row>::new();
    for result in rdr.records() {
        let record = result?;
        let row = record.deserialize(None)?;
        rows.push(row);
    }

    // Find the first date when BTC hits 10000;
    let (first_10000_date, enter_price, enter_index) = {
        let mut date = "Not Found".to_string();
        let mut price = 0.0;
        let mut index = 0;
        for i in 0..rows.len() {
            if rows[i].price >= 10000.0 {
                date = rows[i].date.clone();
                price = rows[i].price;
                index = i;
                break;
            }
        }

        (date, price, index)
    };
    println!("{}, {}", first_10000_date, enter_price);

    // Find the date when BTC price is the lowest.
    let (lowest_price, lowest_date, lowest_date_index) = {
        let mut index = 0;
        let mut lowest_price = rows[index].price;
        for i in 0..rows.len() {
            let price = rows[i].price;
            if price < lowest_price {
                lowest_price = price;
                index = i;
            }
        }

        (lowest_price, rows[index].date.clone(), index)
    };
    println!("lowest {:?}, {:?}", lowest_price, lowest_date);

    const EXIT_CASH_RATIO: f64 = 0.25;
    const REBALANCE_PERCENT_STEPS: f64 = 0.005;
    const BEAR_CASH_RATIO: f64 = 0.75;

    // Invest 90000 into BTC, hold 30000 USD, then immediately cash out using AMM sell algorithm.
    const INIT_CASH: f64 = 30000.0;
    const INIT_COIN_INVEST: f64 = 90000.0;
    let mut cash = INIT_CASH;
    let mut coins = INIT_COIN_INVEST / enter_price;
    let mut sell_logs = vec![];
    let mut price = enter_price;
    let begin_total_asset = cash + INIT_COIN_INVEST;
    println!(
        "begins: {} cash, {} btc, total {}",
        cash, coins, begin_total_asset
    );

    let mut exit_index = enter_index;
    while exit_index < lowest_date_index {
        let current_price = rows[exit_index].price;
        exit_insane_bull(
            EXIT_CASH_RATIO,
            &mut cash,
            &mut coins,
            REBALANCE_PERCENT_STEPS,
            &mut price,
            current_price,
            &mut sell_logs,
        );
        exit_index += 1;
        // When price drops below a point when cash vs btc value == 1:1,
        // assuming that we are in a bear market.
        if cash >= coins * current_price {
            price = current_price;
            break;
        }
    }

    for log in sell_logs.drain(..) {
        println!(
            "At price {} sell {} coins get {} cash",
            log.0,
            -log.1,
            -log.0 * log.1
        );
    }
    println!("remaining: {} cash, {} btc", cash, coins);

    // when price goes back to 10000, how much is the gain?
    let total_asset = cash + coins * enter_price;
    println!(
        "when price dropped back to {}, total {} gain {}",
        enter_price,
        total_asset,
        total_asset - begin_total_asset
    );

    // The investor wants to hold 25% in BTC because he believes in it.
    cash = total_asset * BEAR_CASH_RATIO;
    coins = total_asset * (1.0 - BEAR_CASH_RATIO) / enter_price;
    println!(
        "When price drop to {}, stop profit total cash {}, hold {} in BTC because investor believes in it",
        enter_price, cash, coins,
    );

    // Then BTC dropped to the lowest point.
    println!(
        "Suppose holds, on {} when price dropped to {}, has cash {}, coins worth {}, lost {}",
        rows[exit_index].date,
        price,
        cash,
        coins * price,
        begin_total_asset - cash - coins * price
    );

    // The investor wants to continuously invest into BTC because he believes in it, once the price
    // reaches the bear market price at around 6302.31.
    // Every 30 days he can reserve another 2000 for investment.

    // There are two possibilities after the BTC hits its lowest price: goes down to 300,
    // or a new bull market starts.
    // If BTC goes to 300, execute the buying strategy until the price reaches 300, then see how
    // much money is lost from investing into BTC. But don't stop loss.
    // Compare different buying strategies.

    // If BTC goes back to 13000, see how much money is spent into buying BTC and the average
    // BTC buying price. Compare different buying strategies.
    let mut bull_market_index = rows.len() - 1;
    while bull_market_index > exit_index {
        if rows[bull_market_index].price < 13000.0 {
            break;
        }
        bull_market_index -= 1;
    }
    bull_market_index += 1;

    // Try normal dollar average.
    println!("\n Try normal dollar average:");
    let (_total_bear_invested_cash, _dca_coins, _) = dollar_cost_average(
        bull_market_index,
        exit_index,
        coins,
        lowest_date_index,
        begin_total_asset - cash,
        begin_total_asset,
        &mut DollarCostAverage::default(),
        &rows,
    );
    // Try daily dollar average.
    println!("\n Try daily dollar average:");
    let (_total_bear_invested_cash, _daily_dca_coins, _) = dollar_cost_average(
        bull_market_index,
        exit_index,
        coins,
        lowest_date_index,
        begin_total_asset - cash,
        begin_total_asset,
        &mut DailyDollarCostAverage::default(),
        &rows,
    );
    // Try AMM dollar average.
    println!("\n Try AMM dollar average:");
    let (total_bear_invested_cash, amm_coins, _) = dollar_cost_average(
        bull_market_index,
        exit_index,
        coins,
        lowest_date_index,
        begin_total_asset - cash,
        begin_total_asset,
        &mut AMMCostAverage::new(0.817, 0.8, REBALANCE_PERCENT_STEPS, 150),
        &rows,
    );
    coins = amm_coins;
    println!("\nUse AMM dollar average\n");

    // Rebalance to 1/4 cash, 3/4 coins.
    let bull_start_price = rows[bull_market_index].price;
    let total_asset = cash + coins * bull_start_price;
    let mut start_cash = total_asset * EXIT_CASH_RATIO;
    let mut start_coins = total_asset * (1.0 - EXIT_CASH_RATIO) / bull_start_price;
    println!(
        "Rebalance since bull market starts at price {} on {}, total asset {}, cash {}, coins {}, \
        before rebalance cash {}, coins {}; total cash for invest {}.",
        bull_start_price,
        rows[bull_market_index].date,
        total_asset,
        start_cash,
        start_coins,
        cash,
        coins,
        begin_total_asset + total_bear_invested_cash,
    );

    // After Jan 12 2021, start to exit insane bull market from 33000.
    let mut price = 33000.0;
    println!(
        "begins: {} cash, {} btc, total {}",
        start_cash,
        start_coins,
        start_cash + start_coins * bull_start_price
    );

    let mut exit_index = bull_market_index;
    while exit_index < rows.len() {
        if rows[exit_index].date.starts_with("1/12/2021") {
            break;
        }

        exit_index += 1;
    }
    while exit_index < rows.len() {
        exit_insane_bull(
            EXIT_CASH_RATIO,
            &mut start_cash,
            &mut start_coins,
            REBALANCE_PERCENT_STEPS,
            &mut price,
            rows[exit_index].price,
            &mut sell_logs,
        );
        exit_index += 1;
    }

    for log in sell_logs.drain(..) {
        println!(
            "At price {} sell {} coins get {} cash",
            log.0,
            -log.1,
            -log.0 * log.1
        );
    }
    let total_asset = start_cash + start_coins * rows.last().unwrap().price;
    println!(
        "Last price {} remaining: {} cash, {} btc, total asset {}, unrealized profit {} of {}; \
        cash out precentage {}",
        rows.last().unwrap().price,
        start_cash,
        start_coins,
        total_asset,
        total_asset - (begin_total_asset + total_bear_invested_cash),
        begin_total_asset + total_bear_invested_cash,
        start_cash / (begin_total_asset + total_bear_invested_cash)
    );

    Ok(())
}
