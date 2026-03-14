use crate::state::{Position, TradeStatus};
use chrono::Local;
use std::collections::HashMap;

pub struct RiskManager {
    pub max_per_side: usize,
    pub status: TradeStatus,
    pub hard_stop_pips: f64,
    pub daily_loss_limit: f64,
    pub current_daily_loss: f64,
}

impl RiskManager {
    pub fn new() -> Self {
        Self {
            max_per_side: 3,
            status: TradeStatus::Idle,
            hard_stop_pips: 30.0,
            daily_loss_limit: 1000.0,
            current_daily_loss: 0.0,
        }
    }

    pub fn validate_signal(
        &self,
        side: char,
        probability: f64,
        active_positions: &HashMap<String, Position>,
    ) -> bool {
        if self.current_daily_loss >= self.daily_loss_limit {
            return false;
        }
        if probability < 0.85 {
            return false;
        }

        // Restricción 1: Máximo 3 totales por lado (Buy o Sell)
        let count_side = active_positions.values().filter(|p| p.side == side).count();
        if count_side >= self.max_per_side {
            return false;
        }

        // Restricción 2: Máximo 1 abierta del día en curso (que no esté olvidada)
        let today = Local::now().date_naive();
        let has_active_today = active_positions
            .values()
            .any(|p| p.side == side && p.opened_at.date_naive() == today && !p.is_forgotten);

        if has_active_today {
            return false;
        }

        true
    }

    pub fn calculate_hard_stop(&self, side: char, entry_price: f64) -> f64 {
        let offset = self.hard_stop_pips / 10000.0;
        if side == '1' {
            entry_price - offset
        } else {
            entry_price + offset
        }
    }
}

