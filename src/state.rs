use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy)]
pub struct BookSide {
    pub prices: [f64; 5],
    pub volumes: [f64; 5],
}

impl BookSide {
    pub fn new() -> Self {
        Self {
            prices: [0.0; 5],
            volumes: [0.0; 5],
        }
    }

    pub fn update(&mut self, price: f64, volume: f64, side_type: u8) {
        if volume <= 0.0 {
            self.remove(price);
            return;
        }
        for i in 0..5 {
            if (self.prices[i] - price).abs() < 1e-9 {
                self.volumes[i] = volume;
                return;
            }
        }
        let mut entries: Vec<(f64, f64)> = (0..5)
            .map(|i| (self.prices[i], self.volumes[i]))
            .filter(|(p, _)| *p > 0.0)
            .collect();
        entries.push((price, volume));
        if side_type == 0 {
            entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        } else {
            entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        }
        for i in 0..5 {
            if i < entries.len() {
                self.prices[i] = entries[i].0;
                self.volumes[i] = entries[i].1;
            } else {
                self.prices[i] = 0.0;
                self.volumes[i] = 0.0;
            }
        }
    }

    pub fn remove(&mut self, price: f64) {
        for i in 0..5 {
            if (self.prices[i] - price).abs() < 1e-9 {
                self.prices[i] = 0.0;
                self.volumes[i] = 0.0;
            }
        }
    }
}

pub struct OrderBook {
    pub bids: BookSide,
    pub asks: BookSide,
    pub last_update: u128,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BookSide::new(),
            asks: BookSide::new(),
            last_update: 0,
        }
    }

    pub fn update_from_fix(&mut self, action: char, side: char, price: f64, volume: f64) {
        let side_type = if side == '0' { 0 } else { 1 };
        if action == '2' {
            if side_type == 0 {
                self.bids.remove(price);
            } else {
                self.asks.remove(price);
            }
        } else {
            if side_type == 0 {
                self.bids.update(price, volume, 0);
            } else {
                self.asks.update(price, volume, 1);
            }
        }
        self.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
    }

    pub fn get_mid_price(&self) -> Option<f64> {
        if self.bids.prices[0] > 0.0 && self.asks.prices[0] > 0.0 {
            Some((self.bids.prices[0] + self.asks.prices[0]) / 2.0)
        } else {
            None
        }
    }

    pub fn get_micro_price(&self) -> Option<f64> {
        let (vb, va) = (self.bids.volumes[0], self.asks.volumes[0]);
        if vb + va > 0.0 {
            Some((self.bids.prices[0] * va + self.asks.prices[0] * vb) / (vb + va))
        } else {
            None
        }
    }

    pub fn get_total_liquidity(&self) -> (f64, f64) {
        (
            self.bids.volumes.iter().sum(),
            self.asks.volumes.iter().sum(),
        )
    }
}

