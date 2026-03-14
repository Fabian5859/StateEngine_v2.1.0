use crate::state::OrderBook;
use ndarray::{Array1, Array2};
use std::collections::VecDeque;

pub struct FeatureCollector {
    pub window_size: usize,
    pub data: Vec<Vec<f64>>,
    pub means: Array1<f64>,
    pub stds: Array1<f64>,

    // --- CONTEXTO ADAPTATIVO (SNR) ---
    pub snr_history: VecDeque<f64>,
    pub snr_ema: f64,
    pub ema_alpha: f64, // Calibrado a 0.005 para Fase 4

    // --- TAPE & AGRESIÓN ---
    pub tape_volume_acc: f64,
    pub last_mid_price: f64,

    // --- VOLATILIDAD (Fase 1: ATR) ---
    pub tr_history: VecDeque<f64>,
    pub current_atr: f64,
}

impl FeatureCollector {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            data: Vec::new(),
            means: Array1::zeros(10), // Forzamos 10 dimensiones
            stds: Array1::ones(10),
            snr_history: VecDeque::with_capacity(window_size),
            snr_ema: 0.0,
            ema_alpha: 0.005, // Calibración Fase 4
            tape_volume_acc: 0.0,
            last_mid_price: 0.0,
            tr_history: VecDeque::with_capacity(window_size),
            current_atr: 0.0,
        }
    }

    /// Registra volumen ejecutado real (Tape)
    pub fn add_tape_trade(&mut self, qty: f64) {
        self.tape_volume_acc += qty;
    }

    /// Actualiza el SNR con el suavizado lento de la Fase 4
    pub fn push_snr(&mut self, current_snr: f64) {
        if self.snr_history.len() >= self.window_size {
            self.snr_history.pop_front();
        }
        self.snr_history.push_back(current_snr);

        if self.snr_ema == 0.0 {
            self.snr_ema = current_snr;
        } else {
            // Aplicación del alpha 0.005 para resaltar anomalías
            self.snr_ema = (self.ema_alpha * current_snr) + (1.0 - self.ema_alpha) * self.snr_ema;
        }
    }

    pub fn get_relative_snr_strength(&self, current_snr: f64) -> f64 {
        if self.snr_ema <= 0.0 {
            return 1.0;
        }
        current_snr / self.snr_ema
    }

    /// Implementación de la Fase 1: Masa ponderada cuadrática (1/d^2)
    fn get_weighted_mass(&self, book: &OrderBook, side: char, levels: usize) -> f64 {
        let mut total_weighted_vol = 0.0;
        for i in 0..levels {
            let vol = book.get_volume_at_depth(side, i, i + 1);
            let distance = (i + 1) as f64;
            total_weighted_vol += vol / distance.powi(2);
        }
        total_weighted_vol
    }

    /// Actualiza el ATR para Buckets Dinámicos (Fase 1)
    fn update_atr(&mut self, current_mid: f64) {
        if self.last_mid_price > 0.0 {
            let tr = (current_mid - self.last_mid_price).abs();
            if self.tr_history.len() >= self.window_size {
                self.tr_history.pop_front();
            }
            self.tr_history.push_back(tr);
            self.current_atr = self.tr_history.iter().sum::<f64>() / self.tr_history.len() as f64;
        }
    }

    /// Empaqueta el vector de 10 dimensiones (Fase 4)
    pub fn push_features(&mut self, book: &OrderBook, velocity: f64, noise: f64, current_snr: f64) {
        let mid = book.get_mid_price().unwrap_or(0.0);
        self.update_atr(mid);

        let mut current_row = Vec::with_capacity(10);

        // 1. Log-Return del precio (Momentum)
        let price_change = if self.last_mid_price > 0.0 {
            (mid / self.last_mid_price).ln()
        } else {
            0.0
        };
        current_row.push(price_change);

        // 2. Dinámica de Ticks (Velocity)
        current_row.push(velocity);

        // 3. Ruido de Microestructura
        current_row.push(noise);

        // 4. Fuerza de Señal Relativa (Calibración Fase 4)
        current_row.push(self.get_relative_snr_strength(current_snr));

        // 5. Agresión Pura (Tape Volume)
        current_row.push(self.tape_volume_acc);

        // 6. Imbalance Cercano (L1-L5) ponderado 1/d^2
        let bid_near = self.get_weighted_mass(book, '0', 5);
        let ask_near = self.get_weighted_mass(book, '1', 5);
        current_row.push(bid_near - ask_near);

        // 7. Imbalance Profundo (L6-L20) ponderado 1/d^2
        let bid_deep = self.get_weighted_mass(book, '0', 20);
        let ask_deep = self.get_weighted_mass(book, '1', 20);
        current_row.push(bid_deep - ask_deep);

        // 8. Gap Factor (Liquidity Vacuums)
        let total_near = bid_near + ask_near;
        let total_deep = bid_deep + ask_deep;
        current_row.push(if total_deep > 0.0 {
            total_near / total_deep
        } else {
            1.0
        });

        // 9. ATR (Volatilidad de Microestructura)
        current_row.push(self.current_atr);

        // 10. Ratio Agresión/Liquidez (Crucial para detectar Absorción)
        let total_liq = total_near + total_deep;
        current_row.push(if total_liq > 0.0 {
            self.tape_volume_acc / total_liq
        } else {
            0.0
        });

        // Reset de tape para el siguiente ciclo
        self.tape_volume_acc = 0.0;
        self.last_mid_price = mid;

        // Gestión de memoria
        if self.data.len() >= self.window_size {
            self.data.remove(0);
        }
        self.data.push(current_row);

        if self.data.len() >= 10 {
            self.update_stats();
        }
    }

    fn update_stats(&mut self) {
        let rows = self.data.len();
        let cols = 10; // Fijo para Fase 4
        let mut matrix = Array2::zeros((rows, cols));
        for (i, row) in self.data.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                matrix[[i, j]] = val;
            }
        }
        self.means = matrix.mean_axis(ndarray::Axis(0)).unwrap();
        self.stds = matrix.std_axis(ndarray::Axis(0), 0.0);
        self.stds.mapv_inplace(|x| if x == 0.0 { 1.0 } else { x });
    }

    pub fn get_standardized_vector(&self) -> Array1<f64> {
        if self.data.is_empty() || self.means.len() != 10 {
            return Array1::zeros(0);
        }
        let last_raw = Array1::from_vec(self.data.last().unwrap().clone());
        (last_raw - &self.means) / &self.stds
    }
}

