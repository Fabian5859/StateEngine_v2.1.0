use crate::state::{Position, TradeStatus};
use chrono::Local;

pub struct RiskManager {
    pub max_concurrent_trades: usize,
    pub status: TradeStatus,
    pub hard_stop_pips: f64,      // El "paracaídas" físico (ej. 25 pips)
    pub daily_loss_limit: f64,    // Máxima pérdida diaria permitida
    pub current_daily_loss: f64,
}

impl RiskManager {
    pub fn new(max_ops: usize) -> Self {
        Self {
            max_concurrent_trades: max_ops,
            status: TradeStatus::Idle,
            hard_stop_pips: 25.0,
            daily_loss_limit: 500.0, // Ejemplo: $500
            current_daily_loss: 0.0,
        }
    }

    pub fn validate_signal(
        &self,
        side: char,
        probability: f64,
        active_positions: &Vec<Position>,
    ) -> bool {
        // Bloqueo si excedimos pérdida diaria
        if self.current_daily_loss >= self.daily_loss_limit {
            return false;
        }

        // Regla de Probabilidad (80%)
        if probability < 0.80 {
            return false;
        }

        // Solo SELL para Carry Trade (según tu requerimiento anterior)
        if side != '2' {
            return false;
        }

        // Límite de simultáneas
        if active_positions.len() >= self.max_concurrent_trades {
            return false;
        }

        // Validación Diaria
        let today = Local::now().date_naive();
        let already_traded_today = active_positions
            .iter()
            .any(|pos| pos.opened_at.date_naive() == today);

        if already_traded_today {
            return false;
        }

        true
    }

    /// Calcula el precio del Hard-Stop para enviarlo en el mensaje FIX
    pub fn calculate_hard_stop(&self, side: char, entry_price: f64) -> f64 {
        let offset = self.hard_stop_pips / 10000.0;
        if side == '1' { // Buy
            entry_price - offset
        } else { // Sell
            entry_price + offset
        }
    }

    pub fn set_status(&mut self, status: TradeStatus) {
        self.status = status;
    }

    pub fn get_status(&self) -> TradeStatus {
        self.status
    }
}
