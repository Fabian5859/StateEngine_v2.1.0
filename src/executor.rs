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
    /// Detecta la resistencia del mercado cuando el precio choca contra un muro institucional.
    pub fn evaluate_absorption_test(
        &mut self,
        side: char,
        book: &OrderBook,
        avg_vol: f64,
        current_mid: f64,
    ) -> Option<f64> {
        // 1. Detección de Hueco de Liquidez
        if !book.is_liquidity_gap(side, 5) {
            return None;
        }

        // 2. Identificar el muro institucional en el lado opuesto
        let wall_side_to_look = if side == '1' { '0' } else { '1' };

        if let Some((wall_price, wall_vol)) = book.find_major_wall(wall_side_to_look, avg_vol) {
            let distance = (wall_price - current_mid).abs() * 10000.0;

            // 3. Test de Absorción: El precio llega al muro y este resiste
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

    /// Monitoreo de posiciones para gestión de Salidas (Función Estática para evitar E0502)
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

        // Stop Loss duro de seguridad
        if pips <= -25.0 {
            warn!("🚨 [SL] ID: {} alcanzó límite de riesgo.", pos.cl_ord_id);
            return true;
        }

        // Salida por agotamiento de probabilidad (SNR)
        if pips > 1.5 && current_snr < (snr_avg * 0.6) {
            info!(
                "📉 [SALIDA SNR] Probabilidad agotada ({:.2}). Asegurando {:.1} pips",
                current_snr, pips
            );
            return true;
        }

        // Take Profit Dinámico
        if pips >= 12.0 && current_snr > 0.88 {
            info!("💰 [TP] Objetivo capturado: {:.1} pips", pips);
            return true;
        }

        false
    }

    /// PASO A.4: Gestión de Recovery (Función Estática para evitar E0502)
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

        let cost_to_cover = commission_pips
            + if swap_pips < 0.0 {
                swap_pips.abs()
            } else {
                0.0
            };
        let target = 26.0 + cost_to_cover;

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

