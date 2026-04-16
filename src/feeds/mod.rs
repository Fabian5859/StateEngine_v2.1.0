// Definimos el módulo cqg para que sea accesible
pub mod cqg;

// Aquí podrías añadir otros en el futuro:
// pub mod binance;
// pub mod amp_rithmic;

/// Estructura genérica para estandarizar los trades 
/// que vienen de diferentes fuentes (CQG, Binance, etc.)
#[derive(Debug, Clone)]
pub struct ExternalTrade {
    pub symbol: String,
    pub price: f64,
    pub quantity: u32,
    pub side: String, // "BUY" o "SELL"
    pub source: String,
}
