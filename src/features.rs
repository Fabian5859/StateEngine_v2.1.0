use crate::state::OrderBook;
use std::collections::VecDeque;

pub struct FeatureCollector {
    pub window_size: usize,
    pub snr_history: VecDeque<f64>,
    pub snr_ema: f64,
    // Se elimina tape_volume_acc al no contar con el feed de trades
}

impl FeatureCollector {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            snr_history: VecDeque::with_capacity(window_size),
            snr_ema: 0.0,
        }
    }

    /// Agrega el valor de SNR actual y actualiza el promedio móvil (EMA)
    pub fn push_snr(&mut self, snr: f64) {
        if self.snr_history.len() >= self.window_size {
            self.snr_history.pop_front();
        }
        self.snr_history.push_back(snr);

        // Actualización de EMA (alfa de 0.1 para suavizar)
        if self.snr_ema == 0.0 {
            self.snr_ema = snr;
        } else {
            self.snr_ema = self.snr_ema * 0.9 + snr * 0.1;
        }
    }

    /// Calcula la fuerza relativa del SNR actual respecto a su promedio
    pub fn get_relative_snr_strength(&self, current_snr: f64) -> f64 {
        if self.snr_ema == 0.0 {
            return 1.0;
        }
        current_snr / self.snr_ema
    }

    /// Genera el vector de entrada para el BayesianBrain basado solo en LOB y Velocidad
    /// El vector resultante debe mantener la dimensión esperada por el modelo (ej. 10D)
    pub fn push_features(
        &self,
        book: &OrderBook,
        velocity: f64,
        spread: f64,
        last_snr: f64,
    ) -> Vec<f64> {
        let mut f = Vec::with_capacity(10);

        // 1. Intensidad total del libro
        f.push(book.get_book_intensity());

        // 2. Desequilibrio del libro (Imbalance)
        let total_bids: f64 = book.bids.values().sum();
        let total_asks: f64 = book.asks.values().sum();
        let imbalance = (total_bids - total_asks) / (total_bids + total_asks + 1.0);
        f.push(imbalance);

        // 3. Velocidad de ejecución (Ticks por segundo)
        f.push(velocity);

        // 4. Spread actual normalizado
        f.push(spread * 10000.0);

        // 5. SNR anterior (Feedback del modelo)
        f.push(last_snr);

        // 6. Densidad en el Best Bid
        f.push(*book.bids.values().rev().next().unwrap_or(&0.0));

        // 7. Densidad en el Best Ask
        f.push(*book.asks.values().next().unwrap_or(&0.0));

        // 8. Pendiente del libro (Bid) - Comparación niveles cercanos vs profundos
        let near_bid: f64 = book.bids.values().rev().take(5).sum();
        f.push(near_bid);

        // 9. Pendiente del libro (Ask)
        let near_ask: f64 = book.asks.values().take(5).sum();
        f.push(near_ask);

        // 10. Volatilidad de corto plazo (puedes usar un placeholder o la velocidad corregida)
        f.push(velocity.abs().sqrt());

        f
    }

    /// Estandariza el vector de entrada (Z-Score simplificado o escalado)
    pub fn get_standardized_vector(&self, raw_features: Vec<f64>) -> Vec<f64> {
        // En una implementación real, aquí restarías la media y dividirías por la desviación
        // Por ahora, aplicamos un escalado logarítmico para normalizar magnitudes grandes (como el volumen)
        raw_features
            .into_iter()
            .map(|v| {
                if v.abs() > 1.0 {
                    v.signum() * (1.0 + v.abs().ln())
                } else {
                    v
                }
            })
            .collect()
    }
}

