use chrono::{DateTime, Local};
use std::collections::BTreeMap;

/// Representa el ciclo de vida completo de una orden.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TradeStatus {
    Idle,
    PendingNew,
    New,
    PartiallyFilled,
    Filled,
    Rejected,
}

/// --- Gestión de Posición Activa (V4.0 - Fase 3 & 4) ---
#[derive(Debug, Clone)]
pub struct Position {
    pub cl_ord_id: String,
    pub broker_pos_id: String,
    pub entry_price: f64,
    pub side: char, // '1' Buy, '2' Sell
    pub qty: f64,
    pub opened_at: DateTime<Local>,

    // --- Métricas Bayesianas y de Calidad ---
    pub entry_mu: f64,
    pub probability: f64,

    // --- Seguridad de Vuelo (FASE 3) ---
    pub hard_stop_price: f64, // El paracaídas físico calculado al entrar

    // --- Métricas de Volumen Institucional (FASE 1 & 4) ---
    pub entry_wall_volume: f64,     // Volumen del muro LOB al entrar
    pub entry_tape_aggression: f64, // Volumen real ejecutado (Tape) al entrar
    pub max_adverse_pips: f64,
    pub min_probability_seen: f64,
}

pub struct OrderBook {
    pub bids: BTreeMap<i64, f64>, // Precio escalado (i64) -> Volumen
    pub asks: BTreeMap<i64, f64>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn update(&mut self, action: char, side: char, price: f64, volume: f64) {
        let p_key = (price * 100000.0).round() as i64;
        if side == '0' {
            if action == '2' || volume == 0.0 {
                self.bids.remove(&p_key);
            } else {
                self.bids.insert(p_key, volume);
            }
        } else {
            if action == '2' || volume == 0.0 {
                self.asks.remove(&p_key);
            } else {
                self.asks.insert(p_key, volume);
            }
        }
    }

    // --- FUNCIONES DE ACCESO BÁSICO ---

    pub fn get_mid_price(&self) -> Option<f64> {
        let best_bid = self.bids.keys().rev().next()?;
        let best_ask = self.asks.keys().next()?;
        Some((*best_bid as f64 + *best_ask as f64) / 200000.0)
    }

    pub fn get_best_bid(&self) -> Option<f64> {
        self.bids.keys().rev().next().map(|&p| p as f64 / 100000.0)
    }

    pub fn get_best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|&p| p as f64 / 100000.0)
    }

    // --- FASE 1 & 2: ANÁLISIS DE MICROESTRUCTURA ---

    /// Obtiene el volumen acumulado en un rango específico de niveles.
    pub fn get_volume_at_depth(&self, side: char, start: usize, end: usize) -> f64 {
        if side == '0' {
            self.bids.values().rev().skip(start).take(end - start).sum()
        } else {
            self.asks.values().skip(start).take(end - start).sum()
        }
    }

    /// Calcula la salud de un muro específico (Erosión).
    pub fn get_wall_health(&self, side: char, initial_vol: f64) -> f64 {
        if initial_vol <= 0.0 {
            return 0.0;
        }
        let current_vol = if side == '0' {
            *self.bids.values().rev().next().unwrap_or(&0.0)
        } else {
            *self.asks.values().next().unwrap_or(&0.0)
        };
        current_vol / initial_vol
    }

    /// Detecta un "Vacío de Liquidez" (Liquidity Gap) (Fase 2).
    pub fn is_liquidity_gap_present(&self, side: char, threshold_ratio: f64) -> bool {
        let shock_vol = self.get_volume_at_depth(side, 0, 5);
        let deeper_vol = self.get_volume_at_depth(side, 5, 20);

        if shock_vol <= 0.0 {
            return true;
        }
        (deeper_vol / shock_vol) < threshold_ratio
    }

    /// Encuentra el "Muro Maestro" para el Take Profit Inteligente (Fase 2).
    pub fn find_major_wall(&self, side: char, avg_vol: f64) -> Option<(f64, f64)> {
        let iter: Box<dyn Iterator<Item = (&i64, &f64)>> = if side == '0' {
            Box::new(self.bids.iter().rev())
        } else {
            Box::new(self.asks.iter())
        };

        for (price, vol) in iter.take(30) {
            if *vol > avg_vol * 3.0 {
                return Some((*price as f64 / 100000.0, *vol));
            }
        }
        None
    }

    /// Imbalance del primer nivel (L1)
    pub fn get_imbalance(&self) -> f64 {
        let b_vol = self.bids.values().rev().next().unwrap_or(&0.0);
        let a_vol = self.asks.values().next().unwrap_or(&0.0);
        if b_vol + a_vol == 0.0 {
            return 0.0;
        }
        (b_vol - a_vol) / (b_vol + a_vol)
    }

    /// Métrica de Masa Total (Peso del Libro)
    pub fn get_book_intensity(&self) -> f64 {
        self.bids.values().sum::<f64>() + self.asks.values().sum::<f64>()
    }

    /// Genera un vector de imbalance ponderado para el Cerebro (Fase 4).
    pub fn get_depth_vector(&self, levels: usize) -> Vec<f64> {
        let mut depth_v = Vec::with_capacity(levels);
        let mut bid_iter = self.bids.values().rev();
        let mut ask_iter = self.asks.values();

        for i in 0..levels {
            let b_vol = bid_iter.next().unwrap_or(&0.0);
            let a_vol = ask_iter.next().unwrap_or(&0.0);

            // Ponderación cuadrática inversa para dar más peso a niveles cercanos
            let weight = 1.0 / (i + 1) as f64;

            let imb = if b_vol + a_vol == 0.0 {
                0.0
            } else {
                ((b_vol - a_vol) / (b_vol + a_vol)) * weight
            };
            depth_v.push(imb);
        }
        depth_v
    }
}

