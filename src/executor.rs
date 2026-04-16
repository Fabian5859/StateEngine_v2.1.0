use crate::state::{OrderBook, Position};
use log::{info, warn};
use std::collections::HashMap;

pub struct Executor {
    pub positions: HashMap<String, Position>,
    pub last_wall_volume: f64,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            last_wall_volume: 0.0,
        }
    }

    /// PASO B: Estrategia Test de Absorción (Basada 100% en LOB)
    pub fn evaluate_absorption_test(
        &mut self,
        side: char,
        book: &OrderBook,
        avg_vol: f64,
        current_mid: f64,
    ) -> Option<f64> {
        if !book.is_liquidity_gap(side, 5) {
            return None;
        }

        let wall_side_to_look = if side == '1' { '0' } else { '1' };

        if let Some((wall_price, wall_vol)) = book.find_major_wall(wall_side_to_look, avg_vol) {
            let distance = (wall_price - current_mid).abs() * 10000.0;

            if distance <= 1.2 {
                if self.last_wall_volume > 0.0
                    && wall_vol < self.last_wall_volume
                    && wall_vol > (avg_vol * 2.5)
                {
                    info!(
                        "🎯 [ABSORCIÓN LOB] Muro en {}. Resistencia detectada (Vol: {} -> {}).",
                        wall_price, self.last_wall_volume, wall_vol
                    );
                    self.last_wall_volume = wall_vol;
                    return Some(wall_price);
                }
                self.last_wall_volume = wall_vol;
            }
        } else {
            self.last_wall_volume = 0.0;
        }
        None
    }

    /// Monitoreo de posiciones: FILTRO ESTRICTO DE CERO CIERRES EN PÉRDIDA
    pub fn monitor_position(
        pos: &Position,
        current_mid: f64,
        current_snr: f64,
        snr_avg: f64,
    ) -> bool {
        let pips = if pos.side == '1' {
            (current_mid - pos.entry_price) * 10000.0
        } else {
            (pos.entry_price - current_mid) * 10000.0
        };

        // --- AJUSTE: ELIMINACIÓN DE STOP LOSS FÍSICO / CIERRE EN PÉRDIDA ---
        // Se elimina la condición de pips <= -25.0 para cumplir con la política de no cerrar en negativo.

        // Solo evaluamos salidas si estamos en territorio positivo (pips > 0)
        if pips > 0.0 {
            // 1. Salida por agotamiento de probabilidad (SNR) en Profit
            // Ajustado a 0.5 del promedio para ser más sensible al "olvido" de la tesis
            if pips > 1.0 && current_snr < (snr_avg * 0.5) {
                info!(
                    "📉 [SALIDA PROFIT] SNR debilitado ({:.2}). Cerrando con {:.1} pips",
                    current_snr, pips
                );
                return true;
            }

            // 2. Take Profit Dinámico / Objetivo Fijo
            if pips >= 12.0 && current_snr > 0.88 {
                info!("💰 [TP] Objetivo capturado: {:.1} pips", pips);
                return true;
            }
        }

        false
    }

    /// PASO A.4: Gestión de Recovery
    pub fn check_forgotten_recovery(
        pos: &Position,
        current_mid: f64,
        commission_pips: f64,
        swap_pips: f64,
    ) -> bool {
        if !pos.is_forgotten {
            return false;
        }

        let pips = if pos.side == '1' {
            (current_mid - pos.entry_price) * 10000.0
        } else {
            (pos.entry_price - current_mid) * 10000.0
        };

        // Costo operativo: Comisión + Swap (solo si es negativo para la cuenta)
        let cost_to_cover = commission_pips
            + if swap_pips < 0.0 {
                swap_pips.abs()
            } else {
                0.0
            };

        // Objetivo: 26 puntos (2.6 pips) netos después de costos
        let target = 2.6 + cost_to_cover;

        pips >= target
    }

    /// Manejo de Reportes de Ejecución
    pub fn handle_execution_report(&mut self, msg: &str, pending: &mut Option<Position>) {
        let tags: HashMap<&str, &str> = msg
            .split('|')
            .filter_map(|s| {
                let mut parts = s.splitn(2, '=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        if tags.get("35") == Some(&"8") && tags.get("39") == Some(&"2") {
            if let Some(mut pos) = pending.take() {
                pos.broker_pos_id = tags
                    .get("37")
                    .unwrap_or(tags.get("11").unwrap_or(&"0"))
                    .to_string();
                pos.is_forgotten = false;

                info!(
                    "✅ [EXECUTOR] Sniper Fill: {} | Precio: {}",
                    pos.cl_ord_id, pos.entry_price
                );
                self.positions.insert(pos.cl_ord_id.clone(), pos);
            }
        }
    }
}

