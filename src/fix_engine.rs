use chrono::Utc;
use fefix::prelude::*;
use fefix::tagvalue::{Config, Encoder};
use log::info;

pub struct FixEngine {
    pub encoder: Encoder<Config>,
}

impl FixEngine {
    pub fn new() -> Self {
        info!("Inicializando Motor FEFIX v0.7.0 - Institutional Sniper Safety Config");
        Self {
            encoder: Encoder::<Config>::default(),
        }
    }

    pub fn build_logon(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        sender_sub_id: &str,
        password: &str,
        seq_num: u64,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
        let account_number = sender_id.split('.').last().unwrap_or(sender_id);

        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"A");

        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(50).unwrap(), sender_sub_id.as_bytes());
        msg.set_any(TagU16::new(57).unwrap(), sender_sub_id.as_bytes());

        msg.set_any(TagU16::new(34).unwrap(), format!("{}", seq_num).as_bytes());
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());

        msg.set_any(TagU16::new(98).unwrap(), b"0");
        msg.set_any(TagU16::new(108).unwrap(), b"30");
        msg.set_any(TagU16::new(553).unwrap(), account_number.as_bytes());
        msg.set_any(TagU16::new(554).unwrap(), password.as_bytes());
        msg.set_any(TagU16::new(141).unwrap(), b"Y"); // Reset Sequence para evitar bloqueos

        msg.wrap();
    }

    pub fn build_heartbeat(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        seq_num: u64,
        test_req_id: Option<&str>,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"0");

        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(34).unwrap(), format!("{}", seq_num).as_bytes());
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());

        if let Some(id) = test_req_id {
            msg.set_any(TagU16::new(112).unwrap(), id.as_bytes());
        }

        msg.wrap();
    }

    pub fn build_market_data_request(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        seq_num: u64,
        symbol: &str,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"V");

        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(34).unwrap(), format!("{}", seq_num).as_bytes());
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());
        msg.set_any(TagU16::new(57).unwrap(), b"QUOTE");

        msg.set_any(TagU16::new(262).unwrap(), b"REQ_DEEP_LOB");
        msg.set_any(TagU16::new(263).unwrap(), b"1"); // Snapshot + Updates
        msg.set_any(TagU16::new(264).unwrap(), b"0"); // Full Depth
        msg.set_any(TagU16::new(265).unwrap(), b"1"); // Incremental

        // Petición explícita de tipos de datos (Bid, Ask, Trade)
        msg.set_any(TagU16::new(267).unwrap(), b"3");
        msg.set_any(TagU16::new(269).unwrap(), b"0"); // Bid
        msg.set_any(TagU16::new(269).unwrap(), b"1"); // Ask
        msg.set_any(TagU16::new(269).unwrap(), b"2"); // Trades (TAPE)

        msg.set_any(TagU16::new(146).unwrap(), b"1");
        msg.set_any(TagU16::new(55).unwrap(), symbol.as_bytes());

        msg.wrap();
    }

    pub fn build_order_request(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        seq_num: u64,
        cl_ord_id: &str,
        symbol: &str,
        side: char,
        qty: f64,
        hard_stop_price: f64,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"D");

        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(34).unwrap(), format!("{}", seq_num).as_bytes());
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());
        msg.set_any(TagU16::new(57).unwrap(), b"TRADE");

        msg.set_any(TagU16::new(11).unwrap(), cl_ord_id.as_bytes());
        msg.set_any(TagU16::new(55).unwrap(), symbol.as_bytes());
        msg.set_any(TagU16::new(54).unwrap(), &[side as u8]);
        msg.set_any(TagU16::new(60).unwrap(), now.as_bytes());

        msg.set_any(
            TagU16::new(38).unwrap(),
            format!("{}", qty as u64).as_bytes(),
        );
        msg.set_any(TagU16::new(40).unwrap(), b"1"); // Market
        msg.set_any(TagU16::new(59).unwrap(), b"1"); // GTC
        msg.set_any(
            TagU16::new(99).unwrap(),
            format!("{:.5}", hard_stop_price).as_bytes(),
        );

        msg.wrap();
    }

    pub fn build_close_order(
        &mut self,
        buffer: &mut Vec<u8>,
        sender_id: &str,
        target_id: &str,
        seq_num: u64,
        cl_ord_id: &str,
        symbol: &str,
        side: char,
        qty: f64,
        broker_position_id: &str,
    ) {
        let now = Utc::now().format("%Y%m%d-%H:%M:%S%.3f").to_string();
        buffer.clear();
        let mut msg = self.encoder.start_message(b"FIX.4.4", buffer, b"D");

        msg.set_any(TagU16::new(49).unwrap(), sender_id.as_bytes());
        msg.set_any(TagU16::new(56).unwrap(), target_id.as_bytes());
        msg.set_any(TagU16::new(34).unwrap(), format!("{}", seq_num).as_bytes());
        msg.set_any(TagU16::new(52).unwrap(), now.as_bytes());
        msg.set_any(TagU16::new(57).unwrap(), b"TRADE");

        msg.set_any(TagU16::new(11).unwrap(), cl_ord_id.as_bytes());
        msg.set_any(TagU16::new(55).unwrap(), symbol.as_bytes());
        msg.set_any(TagU16::new(54).unwrap(), &[side as u8]);
        msg.set_any(TagU16::new(60).unwrap(), now.as_bytes());
        msg.set_any(
            TagU16::new(38).unwrap(),
            format!("{}", qty as u64).as_bytes(),
        );
        msg.set_any(TagU16::new(40).unwrap(), b"1");
        msg.set_any(TagU16::new(721).unwrap(), broker_position_id.as_bytes());

        msg.wrap();
    }
}

