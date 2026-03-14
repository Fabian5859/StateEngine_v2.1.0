use ndarray::{Array1, Array2};
use ndarray_rand::RandomExt;
use rand::prelude::*;
use rand_distr::Normal;
use serde::{Deserialize, Serialize};
use std::f64::consts::E;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

#[derive(Debug)]
pub struct BayesianOutput {
    pub mu: f64,
    pub sigma_epistemic: f64,
    pub snr: f64,
}

#[derive(Serialize, Deserialize)]
pub struct BayesianBrain {
    weights1: Array2<f64>,
    weights2: Array1<f64>,
    variance1: Array2<f64>,
    variance2: Array1<f64>,
    learning_rate: f64,
}

impl BayesianBrain {
    pub fn new(input_dim: usize, hidden_dim: usize, lr: f64) -> Self {
        Self {
            // Inicialización Xavier/Glorot implícita con Normal(0, 0.1)
            weights1: Array2::random((input_dim, hidden_dim), Normal::new(0.0, 0.1).unwrap()),
            weights2: Array1::random(hidden_dim, Normal::new(0.0, 0.1).unwrap()),
            variance1: Array2::from_elem((input_dim, hidden_dim), 0.02),
            variance2: Array1::from_elem(hidden_dim, 0.02),
            learning_rate: lr, // Fase 4: 0.005
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        bincode::serialize_into(writer, &self)?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(bincode::deserialize_from(reader)?)
    }

    fn sigmoid(&self, x: f64) -> f64 {
        1.0 / (1.0 + E.powf(-x))
    }

    fn sigmoid_derivative(&self, x: f64) -> f64 {
        let s = self.sigmoid(x);
        s * (1.0 - s)
    }

    /// Inferencia por Monte Carlo (MC Dropout funcional)
    pub fn predict_bayesian(&self, inputs: &Array1<f64>, samples: usize) -> BayesianOutput {
        if inputs.len() != self.weights1.nrows() {
            return BayesianOutput {
                mu: 0.5,
                sigma_epistemic: 1.0,
                snr: 0.0,
            };
        }

        let mut rng = thread_rng();
        let mut predictions = Vec::with_capacity(samples);

        for _ in 0..samples {
            // Muestreo de pesos para capturar incertidumbre epistémica
            let sampled_w2 = Array1::from_shape_fn(self.weights2.len(), |i| {
                let dist =
                    Normal::new(self.weights2[i], self.variance2[i].sqrt().max(1e-6)).unwrap();
                dist.sample(&mut rng)
            });

            let z1 = inputs.dot(&self.weights1);
            let a1 = z1.mapv(|x| self.sigmoid(x));
            let z2 = a1.dot(&sampled_w2);
            predictions.push(self.sigmoid(z2));
        }

        let mu: f64 = predictions.iter().sum::<f64>() / samples as f64;
        let var_e: f64 = predictions.iter().map(|p| (p - mu).powi(2)).sum::<f64>() / samples as f64;
        let sigma_e = var_e.sqrt();

        // SNR: Cuánta señal (distancia de 0.5) tenemos frente a la incertidumbre (sigma)
        let snr = (mu - 0.5).abs() / sigma_e.max(1e-6);

        BayesianOutput {
            mu,
            sigma_epistemic: sigma_e,
            snr,
        }
    }

    pub fn train(&mut self, inputs: &Array1<f64>, target: f64) {
        if inputs.len() != self.weights1.nrows() {
            return;
        }

        let z1 = inputs.dot(&self.weights1);
        let a1 = z1.mapv(|x| self.sigmoid(x));
        let z2 = a1.dot(&self.weights2);
        let prediction = self.sigmoid(z2);

        let error = prediction - target;

        // Retropropagación y ajuste de confianza (Varianza)
        let d_z2 = error * self.sigmoid_derivative(z2);
        for i in 0..self.weights2.len() {
            let grad = d_z2 * a1[i];
            self.weights2[i] -= self.learning_rate * grad;
            // Estabilización: Si el error baja, la varianza (incertidumbre) cae
            self.variance2[i] *= 0.995 + (error.abs() * 0.005);
        }

        let d_a1 = d_z2 * &self.weights2;
        for i in 0..self.weights1.nrows() {
            for j in 0..self.weights1.ncols() {
                let grad = d_a1[j] * self.sigmoid_derivative(z1[j]) * inputs[i];
                self.weights1[[i, j]] -= self.learning_rate * grad;
                self.variance1[[i, j]] *= 0.995 + (error.abs() * 0.005);
            }
        }
    }
}

