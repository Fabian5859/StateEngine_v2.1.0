use log::info; // Eliminado 'error' que generaba warning
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

pub struct BayesianBrain {
    pub weights: Vec<f64>,
    pub learning_rate: f64,
    pub belief_threshold: f64,
    pub history: VecDeque<f64>,
}

impl BayesianBrain {
    pub fn new(input_size: usize) -> Self {
        Self {
            weights: vec![0.0; input_size],
            learning_rate: 0.005,
            belief_threshold: 0.65,
            history: VecDeque::with_capacity(100),
        }
    }

    pub fn load_from_file(path: &str, input_size: usize) -> Self {
        let mut weights = vec![0.0; input_size];
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            for (i, line) in reader.lines().enumerate() {
                if i < input_size {
                    if let Ok(val) = line.unwrap().parse::<f64>() {
                        weights[i] = val;
                    }
                }
            }
            info!("📖 Pesos cargados desde memoria: {:?}", weights);
        } else {
            info!("🆕 No se encontró memoria previa. Iniciando pesos en 0.0");
        }

        Self {
            weights,
            learning_rate: 0.005,
            belief_threshold: 0.65,
            history: VecDeque::with_capacity(100),
        }
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        for w in &self.weights {
            writeln!(file, "{}", w)?;
        }
        Ok(())
    }

    pub fn predict(&self, features: &[f64]) -> f64 {
        let score: f64 = features
            .iter()
            .zip(self.weights.iter())
            .map(|(f, w)| f * w)
            .sum();
        score.tanh()
    }

    pub fn should_trade(&self, prediction: f64) -> (bool, char) {
        if prediction > self.belief_threshold {
            (true, '1') // Buy
        } else if prediction < -self.belief_threshold {
            (true, '2') // Sell
        } else {
            (false, '0')
        }
    }

    pub fn train_with_reward(&mut self, features: &[f64], points: f64, was_forgotten: bool) {
        let reward = if was_forgotten {
            -1.0
        } else if points >= 20.0 {
            1.2
        } else if points >= 15.0 {
            1.0
        } else if points >= 7.0 {
            0.2
        } else {
            0.0
        };

        let prediction = self.predict(features);
        let error_val = reward - prediction;

        for (i, f) in features.iter().enumerate() {
            self.weights[i] += self.learning_rate * error_val * f;
        }
        info!(
            "🧠 Entrenamiento completado. Recompensa: {:.1} | Error: {:.4}",
            reward, error_val
        );
    }

    pub fn audit_weights(&self) {
        let labels = [
            "OFI",
            "MicroPriceDiv",
            "Spread",
            "BidSlope",
            "AskSlope",
            "Phase",
        ];
        let mut audit_msg = String::from("📊 ESTADO DEL MODELO: ");
        for (i, w) in self.weights.iter().enumerate() {
            if i < labels.len() {
                audit_msg.push_str(&format!("{}: {:.4} | ", labels[i], w));
            }
        }
        info!("{}", audit_msg);
    }
}

