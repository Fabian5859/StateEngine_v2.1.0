use log::info;
use std::env;

pub struct RiskManager {
    pub max_buys: usize,
    pub max_sells: usize,
    pub magic_number: u64,
}

impl RiskManager {
    pub fn from_env() -> Self {
        Self {
            max_buys: env::var("MAX_BUY_POSITIONS")
                .unwrap_or("2".to_string())
                .parse()
                .unwrap(),
            max_sells: env::var("MAX_SELL_POSITIONS")
                .unwrap_or("2".to_string())
                .parse()
                .unwrap(),
            magic_number: env::var("MAGIC_NUMBER")
                .unwrap_or("777999".to_string())
                .parse()
                .unwrap(),
        }
    }

    pub fn can_open_more(&self, current_buys: usize, current_sells: usize, side: char) -> bool {
        if side == '1' {
            // BUY
            if current_buys < self.max_buys {
                return true;
            }
            info!(
                "🚫 Límite de COMPRAS alcanzado ({} de {})",
                current_buys, self.max_buys
            );
        } else if side == '2' {
            // SELL
            if current_sells < self.max_sells {
                return true;
            }
            info!(
                "🚫 Límite de VENTAS alcanzado ({} de {})",
                current_sells, self.max_sells
            );
        }
        false
    }
}

