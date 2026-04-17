use crate::features::FeatureCollector;
use crate::state::OrderBook;

#[derive(Debug, Clone)]
pub struct Trade {
    pub id: String,
    pub entry_price: f64,
    pub side: char,
    pub features_at_entry: Vec<f64>,
    pub is_active: bool,
}

pub struct Executor {
    pub trades: Vec<Trade>,
    pub magic_number: u64,
}

impl Executor {
    pub fn new(magic_number: u64) -> Self {
        Self {
            trades: Vec::new(),
            magic_number,
        }
    }

    pub fn manage_hypotheses(&mut self, ob: &OrderBook, collector: &FeatureCollector) {
        for trade in self.trades.iter_mut().filter(|t| t.is_active) {
            if !collector.is_hypothesis_valid(ob, trade.side, trade.features_at_entry[0]) {
                trade.is_active = false;
            }
        }
    }

    pub fn count_positions(&self) -> (usize, usize) {
        let buys = self.trades.iter().filter(|t| t.side == '1').count();
        let sells = self.trades.iter().filter(|t| t.side == '2').count();
        (buys, sells)
    }
}

