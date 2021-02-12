use super::*;

// rebalance_percent_steps: 1%: rebalance for each 1% change of the price.
// finish_price: only sell when start_price is lower than finish price.
// sell_log: a list of (price, amount) to sell.
pub fn exit_insane_bull(
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

pub struct AMMBuyBear {
    pub last_price: f64,
    tick_to_expire: usize,
    pub cash: f64,
    pub coins: f64,

    rebalance_cash_ratio: f64,
    rebalance_step_percentage: f64,
}

impl AMMBuyBear {
    pub fn new(
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

    pub fn check_expiration(&mut self, tick: usize) -> (bool, f64, f64) {
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
    pub fn buy(&mut self, new_price: f64, buy_logs: &mut BuyLogs) -> (f64, f64) {
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
