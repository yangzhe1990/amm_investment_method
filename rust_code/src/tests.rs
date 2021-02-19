// Copyright 2021 Conflux Foundation. All rights reserved.
// Conflux is free software and distributed under GNU General Public License.
// See http://www.gnu.org/licenses/
//
// Modification based on https://github.com/hlb8122/rust-cfx-addr in MIT License.
// A copy of the original license is included in LICENSE.rust-cfx-addr.

use super::amm::*;
use super::cost_average::*;
use super::*;
use cost_average::fluctuation::Fluctuation;

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
        &mut AMMCostAverage::new(0.75, 0.9, REBALANCE_PERCENT_STEPS, 150),
        &rows,
    );
    // Try AMM dollar average auto adjust.
    println!("\n Try AMM dollar average auto adjust:");
    let (_total_bear_invested_cash, amm_coins_2, _) = dollar_cost_average(
        bull_market_index,
        exit_index,
        coins,
        lowest_date_index,
        begin_total_asset - cash,
        begin_total_asset,
        &mut AMMCostAverageAuto::new(0.605, 0.9, REBALANCE_PERCENT_STEPS, 150, 1.0 / 10.0),
        &rows,
    );
    coins = amm_coins;
    println!("\nUse AMM dollar average\n");

    // Fluctuation::new(&rows[exit_index..bull_market_index], &[15, 30, 60, 90, 120, 150]).log();

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
