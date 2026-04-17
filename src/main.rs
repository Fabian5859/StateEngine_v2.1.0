use dotenv::dotenv;
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};
use log::{error, info, warn};
use std::collections::VecDeque;
use std::env;
use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{interval, Duration};

mod brain;
mod executor;
mod features;
mod fix_engine;
mod id_gen;
mod network;
mod risk;
mod state;

use crate::brain::BayesianBrain;
use crate::executor::{Executor, Trade};
use crate::features::FeatureCollector;
use crate::id_gen::IdGenerator;
use crate::risk::RiskManager;
use crate::state::OrderBook;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    // 1. Configuración del Logger de Auditoría
    let _logger = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory("logs")
                .basename("sniper_ic_demo"),
        )
        .write_mode(WriteMode::Async)
        .duplicate_to_stderr(Duplicate::All)
        .start()?;

    info!("🎯 SNIPER V2.1 ACTIVADO - MODO ALTA PERFORMANCE");

    // 2. Inicialización de Componentes
    let weights_path = "sniper_memory.bin";
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let mut brain = BayesianBrain::load_from_file(weights_path, 6);
    let risk_manager = RiskManager::from_env();
    let mut executor = Executor::new(risk_manager.magic_number);
    let mut id_factory = IdGenerator::new();
    let mut engine = crate::fix_engine::FixEngine::new();

    // Cola para el Entrenamiento Pasivo (Shadow Trading)
    let mut prediction_queue: VecDeque<(Vec<f64>, f64)> = VecDeque::with_capacity(150);

    // 3. Variables de Entorno
    let host = env::var("FIX_HOST")?;
    let port_quote = env::var("FIX_PORT_QUOTE")?;
    let port_trade = env::var("FIX_PORT_TRADE")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let password = env::var("FIX_PASSWORD")?;
    let symbol = env::var("FIX_SYMBOL")?;
    let trade_qty: f64 = env::var("TRADE_QTY")?.parse()?;
    let min_profit: f64 = env::var("MIN_PROFIT_POINTS")?.parse()?;

    // 4. Conexión de Sockets
    let mut quote_stream = crate::network::connect_to_broker(&host, &port_quote).await?;
    let mut trade_stream = crate::network::connect_to_broker(&host, &port_trade).await?;
    let mut quote_seq = 1;
    let mut trade_seq = 1;

    // Handshake inicial (Logon)
    let mut logon_q = Vec::new();
    engine.build_logon(
        &mut logon_q,
        &sender_id,
        &target_id,
        "QUOTE",
        &password,
        quote_seq,
    );
    quote_stream.write_all(&logon_q).await?;

    let mut logon_t = Vec::new();
    engine.build_logon(
        &mut logon_t,
        &sender_id,
        &target_id,
        "TRADE",
        &password,
        trade_seq,
    );
    trade_stream.write_all(&logon_t).await?;

    // Suscripción al símbolo configurado
    quote_seq += 1;
    let mut md_req = Vec::new();
    engine.build_market_data_request(&mut md_req, &sender_id, &target_id, quote_seq, &symbol);
    quote_stream.write_all(&md_req).await?;

    let mut hb_timer = interval(Duration::from_secs(25));
    let mut quote_buf = [0u8; 65536];
    let mut trade_buf = [0u8; 16384];

    info!("🚀 Sincronizado. Filtrando por Símbolo ID: {}", symbol);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("🛑 Apagado limpio detectado. Guardando memoria del cerebro...");
                let _ = brain.save_to_file(weights_path);
                break Ok(());
            }

            _ = hb_timer.tick() => {
                quote_seq += 1;
                let mut hb = Vec::new();
                engine.build_heartbeat(&mut hb, &sender_id, &target_id, quote_seq, None);
                let _ = quote_stream.write_all(&hb).await;

                if let Some(mid) = order_book.get_mid_price() {
                    let feats = collector.get_features(&order_book);
                    let pred = brain.predict(&feats);
                    info!("🔎 [AUDIT] Mid: {:.5} | Confianza: {:.4}", mid, pred);
                    brain.audit_weights();
                }
            }

            res_q = quote_stream.read(&mut quote_buf) => {
                let n = res_q?;
                if n == 0 {
                    warn!("🔌 Conexión cerrada por el servidor.");
                    break Ok(());
                }

                let raw_data = String::from_utf8_lossy(&quote_buf[..n]);

                // Procesamos cada mensaje de la ráfaga FIX
                for msg in raw_data.split("8=FIX.4.4") {
                    if msg.is_empty() { continue; }
                    process_lob_fast(msg, &mut order_book, &symbol);
                }

                if let Some(mid) = order_book.get_mid_price() {
                    executor.manage_hypotheses(&order_book, &collector);

                    // --- 1. ENTRENAMIENTO PASIVO ---
                    let features = collector.get_features(&order_book);
                    prediction_queue.push_back((features.clone(), mid));

                    if prediction_queue.len() > 100 {
                        if let Some((old_feats, old_mid)) = prediction_queue.pop_front() {
                            let diff = (mid - old_mid) * 10000.0;
                            if diff.abs() >= 0.1 {
                                let reward = if diff > 0.0 { 1.0 } else { -1.0 };
                                brain.train_with_reward(&old_feats, reward, false);
                            }
                        }
                    }

                    // --- 2. LÓGICA DE CIERRE REAL ---
                    let mut to_remove = Vec::new();
                    for (idx, trade) in executor.trades.iter().enumerate() {
                        let pips = if trade.side == '1' { (mid - trade.entry_price) * 10000.0 }
                                   else { (trade.entry_price - mid) * 10000.0 };

                        if pips >= min_profit {
                            info!("✅ TP: ID {} cerrado con {:.1} pips", trade.id, pips);
                            trade_seq += 1;
                            let mut c_buf = Vec::new();
                            let s_close = if trade.side == '1' { '2' } else { '1' };
                            engine.build_order_request(&mut c_buf, &sender_id, &target_id, trade_seq, &format!("C_{}", trade.id), &symbol, s_close, trade_qty, 0.0);
                            let _ = trade_stream.write_all(&c_buf).await;
                            brain.train_with_reward(&trade.features_at_entry, pips, !trade.is_active);
                            to_remove.push(idx);
                        }
                    }
                    for &idx in to_remove.iter().rev() { executor.trades.remove(idx); }

                    // --- 3. LÓGICA DE ENTRADA REAL ---
                    let prediction = brain.predict(&features);
                    let (should, side) = brain.should_trade(prediction);
                    let (cb, cs) = executor.count_positions();

                    if should && risk_manager.can_open_more(cb, cs, side) {
                        let tid = id_factory.next_id();
                        info!("🚀 DISPARO: {} | Predicción: {:.3} | Lado: {}", tid, prediction, side);
                        trade_seq += 1;
                        let mut o_buf = Vec::new();
                        engine.build_order_request(&mut o_buf, &sender_id, &target_id, trade_seq, &tid, &symbol, side, trade_qty, 0.0);
                        let _ = trade_stream.write_all(&o_buf).await;

                        executor.trades.push(Trade {
                            id: tid, entry_price: mid, side, features_at_entry: features, is_active: true,
                        });
                    }
                }
            }
        }
    }
}

/// Parser Fast: Portero de símbolo al inicio, extracción estricta después.
fn process_lob_fast(msg: &str, order_book: &mut OrderBook, target_symbol: &str) {
    // 1. Portero de Símbolo (Una sola vez por mensaje FIX)
    let sym_pattern = format!("55={}", target_symbol);
    if !msg.contains(&sym_pattern) {
        return;
    }

    // 2. Determinar separador (W=Snapshot, X=Incremental)
    let separator = if msg.contains("35=W") { "269=" } else { "279=" };

    // 3. Iteración sobre niveles (Vía rápida)
    for entry in msg.split(separator).skip(1) {
        let fragment = format!("{}{}", separator, entry);

        let side = extract_tag_fast(&fragment, "269");
        let price = extract_tag_fast(&fragment, "270");
        let vol = extract_tag_fast(&fragment, "271");
        let action = extract_tag_fast(&fragment, "279");

        if let (Some(p), Some(s)) = (price, side) {
            if p > 0.0 && (s == 0.0 || s == 1.0) {
                let side_char = if s == 0.0 { '0' } else { '1' };
                let act_char = match action.unwrap_or(0.0) as i32 {
                    2 => '2', // Delete
                    0 => '0', // New
                    _ => '1', // Update
                };
                order_book.update_from_fix(act_char, side_char, p, vol.unwrap_or(0.0));
            }
        }
    }
}

/// Extractor con delimitadores estrictos para garantizar integridad de datos
fn extract_tag_fast(fragment: &str, tag: &str) -> Option<f64> {
    let pat = format!("{}=", tag);
    if let Some(pos) = fragment.find(&pat) {
        let start = pos + pat.len();
        let sub = &fragment[start..];

        // El valor termina en el siguiente delimitador FIX real
        let end = sub
            .find(|c: char| c == '\x01' || c == '|' || c == '\x00')
            .unwrap_or(sub.len());

        return sub[..end].parse::<f64>().ok();
    }
    None
}

