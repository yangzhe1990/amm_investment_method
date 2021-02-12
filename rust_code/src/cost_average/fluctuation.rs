use Row;

pub struct Fluctuation {
    price_rows: Vec<Row>,
    amm_last_days_analysis: Vec<usize>,
    later_lowest_price: Vec<Vec<f64>>,
    max_later_drop_ratio: Vec<Vec<f64>>,
}

impl Fluctuation {
    pub fn new(price_rows: &[Row], amm_last_days_analysis: &[usize]) -> Self {
        if price_rows.len() == 0 {
            return Self {
                price_rows: vec![],
                amm_last_days_analysis: amm_last_days_analysis.to_vec(),
                later_lowest_price: vec![],
                max_later_drop_ratio: vec![],
            };
        }

        let mut later_lowest_price = vec![vec![]; price_rows.len()];
        let mut max_later_drop_ratio = vec![vec![]; price_rows.len()];
        let price_rows_clone = price_rows.to_vec();

        let mut lowest_price_stack: Vec<(f64, usize)> = vec![];

        let mut index = price_rows.len();

        let mut later_lowest_price_days_stack_index = vec![0; amm_last_days_analysis.len()];

        let mut lowest_price = price_rows.last().unwrap().price;

        let amm_last_days_analysis_len = amm_last_days_analysis.len();

        while index > 0 {
            index -= 1;

            let today_close = price_rows[index].price;
            if today_close < lowest_price {
                lowest_price = today_close;
            }
            later_lowest_price[index].push(lowest_price);
            max_later_drop_ratio[index].push(1.0 - lowest_price / today_close);

            while let Some((price, row_index)) = lowest_price_stack.last().cloned() {
                if today_close < price {
                    lowest_price_stack.pop();
                } else {
                    break;
                }
            }
            lowest_price_stack.push((today_close, index));

            for j in 0..amm_last_days_analysis_len {
                let amm = amm_last_days_analysis[j];
                let mut previous = later_lowest_price_days_stack_index[j];
                if previous >= lowest_price_stack.len() {
                    previous = lowest_price_stack.len() - 1;
                }
                let range_max = index + amm;
                while lowest_price_stack[previous].1 >= range_max {
                    previous += 1;
                }
                later_lowest_price_days_stack_index[j] = previous;

                let lowest_price = lowest_price_stack[previous].0;
                later_lowest_price[index].push(lowest_price);
                max_later_drop_ratio[index].push(1.0 - lowest_price / today_close);
            }
        }

        Self {
            price_rows: price_rows_clone,
            amm_last_days_analysis: amm_last_days_analysis.to_vec(),
            later_lowest_price,
            max_later_drop_ratio,
        }
    }

    pub fn log(&self) {
        println!("log fluctuation");
        let mut days = "inf".to_string();
        for j in 0..self.amm_last_days_analysis.len() + 1 {
            if j > 0 {
                days = self.amm_last_days_analysis[j - 1].to_string();
            }
            for i in 0..self.price_rows.len() {
                println!(
                    "{} {} days range {} later_lowest {} max drop ratio {:.2}%",
                    self.price_rows[i].date,
                    self.price_rows[i].price,
                    days,
                    self.later_lowest_price[i][j],
                    self.max_later_drop_ratio[i][j] * 100.0,
                );
            }
        }
        println!();
    }
}
