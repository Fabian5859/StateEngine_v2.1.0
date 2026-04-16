use crate::state::OrderBook;
use std::collections::VecDeque;

pub struct FeatureCollector {
    pub window_size: usize,
    pub snr_history: VecDeque<f64>,
    pub snr_ema: f64,
    pub tape_volume_acc: f64, // Reintegrado para capturar el feed de trades
}

impl FeatureCollector {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            snr_history: VecDeque::with_capacity(window_size),
            snr_ema: 0.0,
            tape_volume_acc: 0.0,
        }
    }

    /// Acumula el volumen de los trades ejecutados (Tag 269=2)
    pub fn add_tape_trade(&mut self, volume: f64) {
        self.tape_volume_acc += volume;
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

    /// Genera el vector de entrada para el BayesianBrain.
    /// Ahora incluye el Tape Volume (Dipta Das logic).
    pub fn push_features(
        &mut self, // Cambiado a mut para poder resetear el tape_volume_acc
        book: &OrderBook,
        velocity: f64,
        spread: f64,
        last_snr: f64,
    ) -> Vec<f64> {
        let mut f = Vec::with_capacity(10);

        // 1. Intensidad total del libro (Liquidez total)
        f.push(book.get_book_intensity());

        // 2. Desequilibrio del libro (Imbalance de intención)
        let total_bids: f64 = book.bids.values().sum();
        let total_asks: f64 = book.asks.values().sum();
        let imbalance = (total_bids - total_asks) / (total_bids + total_asks + 1.0);
        f.push(imbalance);

        // 3. Velocidad de actualización (Market Activity)
        f.push(velocity);

        // 4. Spread actual normalizado
        f.push(spread * 10000.0);

        // 5. SNR anterior (Feedback del modelo)
        f.push(last_snr);

        // 6. VOLUMEN REAL EJECUTADO (Tape Reading - Dipta Das)
        // Usamos el acumulado desde el último tick y lo reseteamos
        f.push(self.tape_volume_acc);
        self.tape_volume_acc = 0.0;

        // 7. Densidad en el Best Bid
        f.push(*book.bids.values().rev().next().unwrap_or(&0.0));

        // 8. Densidad en el Best Ask
        f.push(*book.asks.values().next().unwrap_or(&0.0));

        // 9. Presión de compra cercana (Top 5 Bids)
        let near_bid: f64 = book.bids.values().rev().take(5).sum();
        f.push(near_bid);

        // 10. Presión de venta cercana (Top 5 Asks)
        let near_ask: f64 = book.asks.values().take(5).sum();
        f.push(near_ask);

        f
    }

    /// Estandariza el vector de entrada (Z-Score simplificado o escalado)
    pub fn get_standardized_vector(&self, raw_features: Vec<f64>) -> Vec<f64> {
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

