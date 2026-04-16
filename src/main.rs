use chrono::Local;
use dotenv::dotenv;
use log::{info, warn};
use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{interval, Duration};

use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming, WriteMode};

// --- MÓDULOS DEL MOTOR ---
mod bayesian;
mod brain;
mod executor;
mod features;
mod feeds;
mod fix_engine;
mod gaussian;
mod id_gen;
mod math_utils;
mod network;
mod risk;
mod state; // Nuevo módulo para feeds externos (CQG, etc.)

use brain::BayesianBrain;
use executor::Executor;
use features::FeatureCollector;
use id_gen::IdGenerator;
use risk::RiskManager;
use state::{OrderBook, Position};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();

    // --- CONFIGURACIÓN DE LOGGER ---
    let _logger = Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default()
                .directory("logs")
                .basename("state_engine"),
        )
        .write_mode(WriteMode::Async)
        .rotate(
            Criterion::Size(10_000_000),
            Naming::Numbers,
            Cleanup::KeepLogFiles(5),
        )
        .duplicate_to_stderr(Duplicate::All)
        .start()?;

    info!("=== STATE ENGINE: SNIPER LOB-ONLY (V5.2 HIGH-PERFORMANCE) ===");

    // --- PARÁMETROS Y ESTADO ---
    let weights_path = "state_engine_brain.bin";
    let trade_qty = env::var("TRADE_QTY")
        .unwrap_or_else(|_| "1000".to_string())
        .parse::<f64>()
        .unwrap_or(1000.0);

    let mut engine = fix_engine::FixEngine::new();
    let mut order_book = OrderBook::new();
    let mut collector = FeatureCollector::new(100);
    let id_factory = IdGenerator::new();
    let risk_manager = RiskManager::new();
    let mut executor = Executor::new();
    let mut quote_seq: u64 = 1;
    let mut trade_seq: u64 = 1;

    let mut brain = BayesianBrain::load_from_file(weights_path)
        .unwrap_or_else(|_| BayesianBrain::new(10, 16, 0.005));

    let mut last_velocity_calc = Instant::now();
    let mut tick_count = 0.0;
    let mut current_velocity = 0.0;
    let mut pending_thesis: Option<Position> = None;
    let mut prediction_queue: VecDeque<(Vec<f64>, f64)> = VecDeque::new();

    // --- CONFIGURACIÓN FIX (IC MARKETS) ---
    let host = env::var("FIX_HOST")?;
    let sender_id = env::var("FIX_SENDER_ID")?;
    let target_id = env::var("FIX_TARGET_ID")?;
    let password = env::var("FIX_PASSWORD")?;
    let port_quote = env::var("FIX_PORT_QUOTE")?;
    let port_trade = env::var("FIX_PORT_TRADE")?;
    let symbol = env::var("FIX_SYMBOL").unwrap_or_else(|_| "EURUSD".to_string());

    // --- LANZAMIENTO DE FEEDS EXTERNOS (CQG) ---
    // Usamos spawn para que corra en su propio hilo asíncrono
    tokio::spawn(async move {
        // ID de tu demo de CQG activa
        feeds::cqg::connect_to_cqg("PSCQCDemo10544").await;
    });

    'reconnect: loop {
        info!("🔌 Conectando a cTrader FIX Gateway...");

        let mut quote_stream = match network::connect_to_broker(&host, &port_quote).await {
            Ok(s) => s,
            Err(e) => {
                warn!("❌ Error QUOTE: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue 'reconnect;
            }
        };

        let mut trade_stream = match network::connect_to_broker(&host, &port_trade).await {
            Ok(s) => s,
            Err(e) => {
                warn!("❌ Error TRADE: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue 'reconnect;
            }
        };

        // Logon Inicial FIX
        let mut buf_q = Vec::new();
        engine.build_logon(
            &mut buf_q, &sender_id, &target_id, "QUOTE", &password, quote_seq,
        );
        quote_stream.write_all(&buf_q).await?;

        let mut buf_t = Vec::new();
        engine.build_logon(
            &mut buf_t, &sender_id, &target_id, "TRADE", &password, trade_seq,
        );
        trade_stream.write_all(&buf_t).await?;

        quote_seq += 1;
        let mut md_req = Vec::new();
        engine.build_market_data_request(&mut md_req, &sender_id, &target_id, quote_seq, &symbol);
        quote_stream.write_all(&md_req).await?;

        let mut hb_timer_q = interval(Duration::from_secs(25));
        let mut hb_timer_t = interval(Duration::from_secs(25));
        let mut quote_buf = [0u8; 32768];
        let mut trade_buf = [0u8; 16384];

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("🛑 Guardando Cerebro y Apagando...");
                    let _ = brain.save_to_file(weights_path);
                    return Ok(());
                }
                _ = hb_timer_q.tick() => {
                    quote_seq += 1;
                    let mut hb = Vec::new();
                    engine.build_heartbeat(&mut hb, &sender_id, &target_id, quote_seq, None);
                    let _ = quote_stream.write_all(&hb).await;
                }
                _ = hb_timer_t.tick() => {
                    trade_seq += 1;
                    let mut hb = Vec::new();
                    engine.build_heartbeat(&mut hb, &sender_id, &target_id, trade_seq, None);
                    let _ = trade_stream.write_all(&hb).await;
                }
                res_t = trade_stream.read(&mut trade_buf) => {
                    if let Ok(n) = res_t {
                        if n == 0 { break; }
                        let msg = String::from_utf8_lossy(&trade_buf[..n]).replace('\x01', "|");
                        executor.handle_execution_report(&msg, &mut pending_thesis);
                    }
                }
                res_q = quote_stream.read(&mut quote_buf) => {
                    if let Ok(n) = res_q {
                        if n == 0 { break; }
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

                                    process_lob_message(full_msg, &mut order_book, &mut tick_count, &mut collector);
                                    search_pos = msg_start + msg_len;
                                } else { break; }
                            } else { break; }
                        }

                        // --- FEATURE ENGINEERING Y PREDICCIÓN ---
                        if let Some(mid) = order_book.get_mid_price() {
                            let elapsed = last_velocity_calc.elapsed().as_secs_f64();
                            if elapsed >= 1.0 {
                                current_velocity = tick_count / elapsed;
                                info!("📊 [HEALTH] Mid: {:.5} | Ticks/s: {:.0} | Tape Vol: {:.0} | B:{} A:{}",
                                     mid, current_velocity, collector.tape_volume_acc, order_book.bids.len(), order_book.asks.len());
                                tick_count = 0.0;
                                last_velocity_calc = Instant::now();
                            }

                            let raw_v = collector.push_features(&order_book, current_velocity, 0.00001, collector.snr_ema);
                            let norm_v = collector.get_standardized_vector(raw_v);

                            if !norm_v.is_empty() {
                                let b_out = brain.predict_bayesian(&norm_v, 20);
                                collector.push_snr(b_out.snr);

                                // Gestión de posiciones existentes
                                let mut to_close = Vec::new();
                                for (id, pos) in executor.positions.iter_mut() {
                                    let pips = if pos.side == '1' { (mid - pos.entry_price) * 10000.0 } else { (pos.entry_price - mid) * 10000.0 };
                                    if !pos.is_forgotten && pips <= -15.0 {
                                        pos.is_forgotten = true;
                                        brain.train(&pos.entry_features, 0.0);
                                    }
                                    if Executor::monitor_position(pos, mid, b_out.snr, collector.snr_ema) {
                                        to_close.push(id.clone());
                                        if pips > 0.0 { brain.train(&pos.entry_features, 1.0); }
                                    }
                                }

                                for id in to_close {
                                    if let Some(pos) = executor.positions.remove(&id) {
                                        trade_seq += 1;
                                        let mut exit_buf = Vec::new();
                                        let side_close = if pos.side == '1' { '2' } else { '1' };
                                        engine.build_close_order(&mut exit_buf, &sender_id, &target_id, trade_seq, &id_factory.next_id(), &symbol, side_close, pos.qty, &pos.broker_pos_id);
                                        let _ = trade_stream.write_all(&exit_buf).await;
                                    }
                                }

                                // --- DETECCIÓN DE ABSORCIÓN SNIPER ---
                                for side in ['1', '2'] {
                                    if risk_manager.validate_signal(side, b_out.snr, &executor.positions) {
                                        if let Some(limit_price) = executor.evaluate_absorption_test(side, &order_book, 60.0, mid) {
                                            let cl_id = id_factory.next_id();
                                            let h_stop = risk_manager.calculate_hard_stop(side, limit_price);
                                            let new_pos = Position {
                                                cl_ord_id: cl_id.clone(), broker_pos_id: String::new(), entry_price: limit_price,
                                                side, qty: trade_qty, opened_at: Local::now(), is_forgotten: false,
                                                entry_mu: b_out.mu, probability: b_out.snr, hard_stop_price: h_stop,
                                                max_adverse_pips: 0.0, entry_features: norm_v.clone(),
                                            };
                                            trade_seq += 1;
                                            let mut buf = Vec::new();
                                            engine.build_order_request(&mut buf, &sender_id, &target_id, trade_seq, &cl_id, &symbol, side, trade_qty, h_stop);
                                            let _ = trade_stream.write_all(&buf).await;
                                            pending_thesis = Some(new_pos);
                                        }
                                    }
                                }

                                prediction_queue.push_back((norm_v.clone(), mid));
                                if prediction_queue.len() > 10 {
                                    if let Some((old_f, old_p)) = prediction_queue.pop_front() {
                                        let target = if mid > old_p { 1.0 } else { 0.0 };
                                        brain.train(&old_f, target);
                                    }
                                }
                            }
                        }
                    } else { break; }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

// --- UTILIDADES DE PROCESAMIENTO FIX ---

fn process_lob_message(
    msg: &str,
    order_book: &mut OrderBook,
    tick_count: &mut f64,
    collector: &mut FeatureCollector,
) {
    let sym_id = extract_tag_string(msg, "55").unwrap_or_else(|| "N/A".to_string());
    if sym_id != "1" && sym_id != "EURUSD" {
        return;
    }

    let separator = if msg.contains("35=W") { "269=" } else { "279=" };
    for entry in msg.split(separator).skip(1) {
        let fragment = format!("{}{}", separator, entry);
        let side = extract_tag_val(&fragment, "269").unwrap_or(-1.0);
        let price = extract_tag_val(&fragment, "270").unwrap_or(0.0);
        let vol = extract_tag_val(&fragment, "271").unwrap_or(0.0);
        let act = extract_tag_val(&fragment, "279").unwrap_or(0.0);

        if side == 2.0 {
            collector.add_tape_trade(vol);
            info!("🎯 [TAPE] P:{} V:{}", price, vol);
        } else if side >= 0.0 {
            let s = if side == 0.0 { '0' } else { '1' };
            let a = if act == 2.0 { '2' } else { '1' };
            order_book.update(a, s, price, vol);
            *tick_count += 1.0;
        }
    }
}

fn extract_tag_val(msg: &str, tag: &str) -> Option<f64> {
    let pat = format!("{}=", tag);
    if let Some(s) = msg.find(&pat) {
        let start = s + pat.len();
        let frag = &msg[start..];
        let end = frag
            .find('\x01')
            .or_else(|| frag.find('|'))
            .unwrap_or(frag.len());
        return frag[..end].parse::<f64>().ok();
    }
    None
}

fn extract_tag_string(msg: &str, tag: &str) -> Option<String> {
    let pat = format!("{}=", tag);
    msg.find(&pat).map(|s| {
        let start = s + pat.len();
        let frag = &msg[start..];
        let end = frag
            .find('\x01')
            .or_else(|| frag.find('|'))
            .unwrap_or(frag.len());
        frag[..end].to_string()
    })
}
