use crate::state::OrderBook;

pub struct FeatureCollector {
    pub window_size: usize,
}

impl FeatureCollector {
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }

    pub fn get_features(&self, ob: &OrderBook) -> Vec<f64> {
        let mut f = Vec::new();
        let (bm, am) = ob.get_total_liquidity();
        f.push(if bm + am > 0.0 {
            (bm - am) / (bm + am)
        } else {
            0.0
        }); // OFI
        if let (Some(mid), Some(micro)) = (ob.get_mid_price(), ob.get_micro_price()) {
            f.push(((micro - mid) * 10000.0).clamp(-1.0, 1.0));
        } else {
            f.push(0.0);
        }
        let spread = (ob.asks.prices[0] - ob.bids.prices[0]) * 10000.0;
        f.push(spread.clamp(0.0, 1.0));
        f.push((ob.bids.prices[0] - ob.bids.prices[4]).clamp(0.0, 0.001) * 1000.0);
        f.push((ob.asks.prices[4] - ob.asks.prices[0]).clamp(0.0, 0.001) * 1000.0);
        f.push(0.0); // Placeholder para balancear a 6 features
        f
    }

    pub fn is_hypothesis_valid(&self, ob: &OrderBook, side: char, _entry_ofi: f64) -> bool {
        let (bm, am) = ob.get_total_liquidity();
        let ofi = if bm + am > 0.0 {
            (bm - am) / (bm + am)
        } else {
            0.0
        };
        if side == '1' {
            ofi > -0.1
        } else {
            ofi < 0.1
        }
    }
}

