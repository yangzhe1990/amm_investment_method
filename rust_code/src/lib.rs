// Copyright 2021 Conflux Foundation. All rights reserved.
// Conflux is free software and distributed under GNU General Public License.
// See http://www.gnu.org/licenses/
//
// Modification based on https://github.com/hlb8122/rust-bitcoincash-addr in MIT License.
// A copy of the original license is included in LICENSE.rust-bitcoincash-addr.

extern crate csv;
#[macro_use]
extern crate serde_derive;
extern crate serde;

pub mod amm;
pub mod cost_average;

#[derive(Clone, Deserialize, Debug)]
pub struct Row {
    date: String,
    price: f64,
}

type BuyLogs = Vec<(f64, f64)>;

#[cfg(test)]
mod tests;
