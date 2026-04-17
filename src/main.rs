use dotenv::dotenv;
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode};
use log::{info, warn}; // Eliminado 'error'
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

    let _logger = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory("logs")
                .basename("sniper_ic_live"),
        )
        .write_mode(WriteMode::Async)
        .duplicate_to_stderr(Duplicate::All)
        .start()?;

    info!("🎯 SNIPER V2.1 ACTIVADO - UNIFICADO (PARSER REFERENCIA + IA)");

    let weights_path = "sniper_memory.bin";
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let mut brain = BayesianBrain::load_from_file(weights_path, 6);
    let risk_manager = RiskManager::from_env();
    let mut executor = Executor::new(risk_manager.magic_number);
    let mut id_factory = IdGenerator::new();
    let mut engine = crate::fix_engine::FixEngine::new();

    let mut prediction_queue: VecDeque<(Vec<f64>, f64)> = VecDeque::with_capacity(150);

    let host = env::var("FIX_HOST")?;
    let port_quote = env::var("FIX_PORT_QUOTE")?;
    let port_trade = env::var("FIX_PORT_TRADE")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let password = env::var("FIX_PASSWORD")?;
    let symbol = env::var("FIX_SYMBOL")?;
    let trade_qty: f64 = env::var("TRADE_QTY")?.parse()?;
    let min_profit: f64 = env::var("MIN_PROFIT_POINTS")?.parse()?;

    let mut quote_stream = crate::network::connect_to_broker(&host, &port_quote).await?;
    let mut trade_stream = crate::network::connect_to_broker(&host, &port_trade).await?;
    let mut quote_seq = 1;
    let mut trade_seq = 1;

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

    quote_seq += 1;
    let mut md_req = Vec::new();
    engine.build_market_data_request(&mut md_req, &sender_id, &target_id, quote_seq, &symbol);
    quote_stream.write_all(&md_req).await?;

    let mut hb_timer = interval(Duration::from_secs(25));
    let mut quote_buf = [0u8; 65536];

    info!("🚀 Sistema en línea. Monitoreando: {}", symbol);

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
                    // LÍNEA 125 CORREGIDA AQUÍ:
                    info!("🔎 [AUDIT] Mid: {:.5} | Confianza: {:.4} | B/A: {}/{}",
                        mid, pred, order_book.bids.prices.len(), order_book.asks.prices.len());
                    brain.audit_weights();
                }
            }

            res_q = quote_stream.read(&mut quote_buf) => {
                let n = res_q?;
                if n == 0 {
                    warn!("🔌 Conexión perdida.");
                    break Ok(());
                }

                let raw = String::from_utf8_lossy(&quote_buf[..n]);

                let mut search_pos = 0;
                while let Some(start) = raw[search_pos..].find("8=FIX.4.4") {
                    let msg_start = search_pos + start;
                    let rest = &raw[msg_start..];

                    if let Some(chk_pos) = rest.find("\x0110=") {
                        let after_chk = &rest[chk_pos + 4..];
                        if let Some(final_sep) = after_chk.find('\x01') {
                            let msg_len = chk_pos + 4 + final_sep + 1;
                            let full_msg = &rest[..msg_len];

                            process_lob_fast(full_msg, &mut order_book, &symbol);

                            search_pos = msg_start + msg_len;
                        } else { break; }
                    } else { break; }
                }

                if let Some(mid) = order_book.get_mid_price() {
                    executor.manage_hypotheses(&order_book, &collector);

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

                    let prediction = brain.predict(&features);
                    let (should, side) = brain.should_trade(prediction);
                    let (cb, cs) = executor.count_positions();

                    if should && risk_manager.can_open_more(cb, cs, side) {
                        let tid = id_factory.next_id();
                        info!("🚀 DISPARO: {} | Pred: {:.3} | Lado: {}", tid, prediction, side);
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

fn process_lob_fast(msg: &str, order_book: &mut OrderBook, target_symbol: &str) {
    let sym_match = format!("55={}", target_symbol);
    if !msg.contains(&sym_match) && !msg.contains("55=1") && !msg.contains("55=EURUSD") {
        return;
    }

    let separator = if msg.contains("35=W") { "269=" } else { "279=" };

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

fn extract_tag_fast(fragment: &str, tag: &str) -> Option<f64> {
    let pat = format!("{}=", tag);
    if let Some(pos) = fragment.find(&pat) {
        let start = pos + pat.len();
        let sub = &fragment[start..];
        let end = sub
            .find(|c: char| c == '\x01' || c == '|' || c == '\x00')
            .unwrap_or(sub.len());

        return sub[..end].parse::<f64>().ok();
    }
    None
}

