use chrono::{DateTime, Local};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TradeStatus {
    Idle,
    PendingNew,
    New,
    PartiallyFilled,
    Filled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub cl_ord_id: String,
    pub broker_pos_id: String,
    pub entry_price: f64,
    pub side: char, // '1' Buy, '2' Sell
    pub qty: f64,
    pub opened_at: DateTime<Local>,
    pub is_forgotten: bool,
    pub entry_mu: f64,
    pub probability: f64,
    pub hard_stop_price: f64,
    pub max_adverse_pips: f64,
    pub entry_features: Vec<f64>, // Guardamos las condiciones de mercado al entrar para el aprendizaje (Paso C)
}

pub struct OrderBook {
    pub bids: BTreeMap<i64, f64>,
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

    pub fn get_mid_price(&self) -> Option<f64> {
        let best_bid = self.bids.keys().rev().next()?;
        let best_ask = self.asks.keys().next()?;
        Some((*best_bid as f64 + *best_ask as f64) / 200000.0)
    }

    /// Detecta si hay un hueco de liquidez (poca densidad) en los próximos X pips.
    /// Clave para la estrategia de Absorción (Paso B).
    pub fn is_liquidity_gap(&self, side: char, pips: i64) -> bool {
        let mid_price = match self.get_mid_price() {
            Some(p) => p,
            None => return false,
        };
        let mid_key = (mid_price * 100000.0).round() as i64;
        let mut density = 0.0;

        if side == '1' {
            // Mirando hacia arriba para un Sell
            for i in 1..=pips {
                density += self.asks.get(&(mid_key + i)).unwrap_or(&0.0);
            }
        } else {
            // Mirando hacia abajo para un Buy
            for i in 1..=pips {
                density += self.bids.get(&(mid_key - i)).unwrap_or(&0.0);
            }
        }
        // Un gap se define si el volumen total en el rango es menor a un umbral mínimo
        density < 5.0
    }

    /// Busca muros institucionales significativos en el libro de órdenes.
    pub fn find_major_wall(&self, side: char, avg_vol: f64) -> Option<(f64, f64)> {
        let iter: Box<dyn Iterator<Item = (&i64, &f64)>> = if side == '0' {
            Box::new(self.bids.iter().rev())
        } else {
            Box::new(self.asks.iter())
        };

        // Escaneamos los niveles más cercanos al precio actual
        for (price, vol) in iter.take(20) {
            // Un muro se considera mayor si es al menos 4 veces el promedio de volumen del libro
            if *vol > avg_vol * 4.0 {
                return Some((*price as f64 / 100000.0, *vol));
            }
        }
        None
    }

    /// Calcula la profundidad total o intensidad del libro (útil como feature para el BayesianBrain)
    pub fn get_book_intensity(&self) -> f64 {
        self.bids.values().sum::<f64>() + self.asks.values().sum::<f64>()
    }
}



