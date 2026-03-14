use crate::risk::RiskManager;
use crate::state::{OrderBook, Position, TradeStatus};
use chrono::Local;
use log::{error, info, warn};

pub struct Executor {
    pub active_position: Option<Position>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            active_position: None,
        }
    }

    /// LÓGICA SNIPER HÍBRIDA (Fase 2 & 3)
    pub fn evaluate_sniper_entry(
        &self,
        side: char,
        order_book: &OrderBook,
        initial_wall_vol: f64,
        current_snr: f64,
        avg_snr: f64,
        tape_speed: f64,
    ) -> Option<bool> {
        // --- 1. FILTRO DE CONTEXTO (SNR) ---
        // Exigimos 20% más que el promedio lento (Fase 4)
        if current_snr < (avg_snr * 1.2) {
            return None;
        }

        let wall_side = if side == '1' { '1' } else { '0' };
        let wall_health = order_book.get_wall_health(wall_side, initial_wall_vol);

        // --- 2. FILTRO DE AGRESIÓN (ANTI-SPOOFING) ---
        if wall_health <= 0.20 {
            let lob_delta = initial_wall_vol - (initial_wall_vol * wall_health);

            // Verificamos si la desaparición del muro fue por trades reales
            let aggression_ratio = if lob_delta > 0.0 {
                tape_speed / lob_delta
            } else {
                0.0
            };

            if aggression_ratio < 0.70 {
                warn!(
                    "🚫 [SPOOFING] Muro retirado sin trades (Ratio: {:.2}). Abortando Sniper.",
                    aggression_ratio
                );
                return None;
            }

            // --- 3. ZONA DE ESCAPE (Fase 2: MARKET VS LIMIT) ---
            let is_vacuum = order_book.is_liquidity_gap_present(wall_side, 0.3);

            if is_vacuum {
                info!(
                    "🚀 [SNIPER-MARKET] Vacío tras muro. Agresión: {:.2}x",
                    aggression_ratio
                );
                return Some(true);
            } else {
                info!(
                    "⚖️ [SNIPER-LIMIT] Densidad detectada. Retest probable. Agresión: {:.2}x",
                    aggression_ratio
                );
                return Some(false);
            }
        }
        None
    }

    pub fn monitor_position(
        &mut self,
        current_mid: f64,
        current_snr: f64,
        avg_snr: f64,
        current_mu: f64,
        order_book: &OrderBook,
        _risk_manager: &mut RiskManager,
        tape_speed: f64,
    ) -> bool {
        let pos = match &mut self.active_position {
            Some(p) => p,
            None => return false,
        };

        let multiplier = 10000.0;
        let current_pips = if pos.side == '2' {
            (pos.entry_price - current_mid) * multiplier
        } else {
            (current_mid - pos.entry_price) * multiplier
        };

        // Actualización de métricas de riesgo
        let adverse_move = if pos.side == '2' {
            (current_mid - pos.entry_price).max(0.0) * multiplier
        } else {
            (pos.entry_price - current_mid).max(0.0) * multiplier
        };
        if adverse_move > pos.max_adverse_pips {
            pos.max_adverse_pips = adverse_move;
        }
        if current_snr < pos.min_probability_seen {
            pos.min_probability_seen = current_snr;
        }

        let snr_strength = if avg_snr > 0.0 {
            current_snr / avg_snr
        } else {
            1.0
        };

        // --- 1. DETECCIÓN DE ABSORCIÓN (Iceberg en contra) ---
        if current_pips > 0.0 && current_pips < 0.4 && tape_speed > (pos.qty * 3.0) {
            warn!("🧊 [ABSORCIÓN] Volumen masivo detectado contra nosotros. Posible Iceberg. Saliendo.");
            return true;
        }

        // --- 2. INVALIDEZ ESTADÍSTICA (Fase 4) ---
        let mu_reversed = if pos.side == '2' {
            current_mu > 0.65
        } else {
            current_mu < 0.35
        };
        if snr_strength < 0.35 || mu_reversed {
            warn!("⚠️ [EXIT] SNR degradado o dirección revertida. Tesis invalidada.");
            return true;
        }

        // --- 3. GESTIÓN DE ESCUDO (LOB Dinámico) ---
        if current_pips <= -4.0 {
            let shield_side = if pos.side == '2' { '1' } else { '0' };
            let avg_vol = order_book.get_book_intensity() / 40.0;

            if let Some((wall_price, wall_vol)) = order_book.find_major_wall(shield_side, avg_vol) {
                let dist = (wall_price - current_mid).abs() * multiplier;
                if dist < 2.5 {
                    info!(
                        "🛡️ [ESCUDO] Muro de {:.1} lotes a {:.1} pips. Manteniendo.",
                        wall_vol, dist
                    );
                } else if current_pips <= -8.0 {
                    error!("🚨 [SL] Sin escudo cercano y -8 pips. Cierre de seguridad.");
                    return true;
                }
            } else if current_pips <= -6.0 {
                warn!("💨 [EXIT] Sin protección en el LOB (Vacío). Salida preventiva.");
                return true;
            }
        }

        // --- 4. TAKE PROFIT POR AGOTAMIENTO ---
        if current_pips >= 12.0 {
            let target_side = if pos.side == '2' { '0' } else { '1' };
            let avg_vol = order_book.get_book_intensity() / 40.0;
            if let Some((_, wall_vol)) = order_book.find_major_wall(target_side, avg_vol) {
                if wall_vol > pos.qty * 5.0 {
                    info!(
                        "🎯 [TP] Chocando con liquidez institucional ({:.1} lotes).",
                        wall_vol
                    );
                    return true;
                }
            }
        }

        // --- 5. LIMITES DUROS (Hard Limits) ---
        if current_pips >= 18.0 {
            info!("💰 [TP-MAX] +18 pips.");
            return true;
        }
        if current_pips <= -25.0 {
            error!("💀 [PANIC] Hard-Stop físico alcanzado.");
            return true;
        }

        false
    }

    pub fn handle_execution_report(
        &mut self,
        msg: &str,
        risk: &mut RiskManager,
        pending: &mut Option<Position>,
    ) {
        let tags: std::collections::HashMap<&str, &str> = msg
            .split('|')
            .filter_map(|s| {
                let mut parts = s.splitn(2, '=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        if tags.get("35") == Some(&"8") && tags.get("39") == Some(&"2") {
            if let Some(mut thesis) = pending.take() {
                // Seteamos el ID del broker antes del movimiento
                thesis.broker_pos_id = tags
                    .get("37")
                    .or(tags.get("11"))
                    .unwrap_or(&"0")
                    .to_string();
                thesis.opened_at = Local::now();
                risk.set_status(TradeStatus::Filled);

                // CORRECCIÓN E0382: Imprimimos ANTES de mover 'thesis' a self.active_position
                info!(
                    "✅ [EXECUTOR] Sniper en posición. ID: {}",
                    thesis.broker_pos_id
                );

                // Ahora movemos el objeto (el ownership pasa al Executor)
                self.active_position = Some(thesis);
            }
        } else if tags.get("35") == Some(&"8")
            && (tags.get("39") == Some(&"8") || tags.get("39") == Some(&"4"))
        {
            risk.set_status(TradeStatus::Idle);
            pending.take();
        }
    }
}

