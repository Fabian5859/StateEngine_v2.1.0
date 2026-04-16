use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use prost::Message as SimpleMessage;
use std::env;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

// Invocamos el código generado por prost-build
pub mod market_data {
    include!(concat!(env!("OUT_DIR"), "/cqg.rs"));
}

/// Función principal que gestiona el ciclo de vida de la conexión con CQG
pub async fn connect_to_cqg(user_id_fallback: &str) {
    let user = env::var("CQG_USER").unwrap_or_else(|_| user_id_fallback.to_string());
    let password = env::var("CQG_PASSWORD").unwrap_or_default();
    let app_id = env::var("CQG_APP_ID").unwrap_or_else(|_| "StateEngine_v2".to_string());

    let url_str = "wss://demoapi.cqg.com:443";

    loop {
        info!("--- FEED EXTERNO: INTENTANDO CONEXIÓN ---");

        match connect_async(url_str).await {
            Ok((mut ws_stream, _)) => {
                info!("✅ Canal WebSocket abierto con éxito hacia Chicago");

                // 1. CONSTRUIR Y ENVIAR LOGON INMEDIATAMENTE
                let logon = market_data::Logon {
                    user_name: user.clone(),
                    password: password.clone(),
                    application_id: app_id.clone(),
                    client_version: "StateEngine_v2.1.0".to_string(),
                };

                let client_msg = market_data::ClientMsg {
                    msg_type: Some(market_data::client_msg::MsgType::Logon(logon)),
                };

                let mut buf = Vec::new();
                if let Ok(_) = client_msg.encode(&mut buf) {
                    if let Ok(_) = ws_stream.send(Message::Binary(buf.into())).await {
                        info!("🚀 Solicitud de Logon enviada a CQG para {}", user);
                    }
                }

                // 2. BUCLE DE ESCUCHA Y PROCESAMIENTO
                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(Message::Binary(bin)) => {
                            if let Ok(server_msg) = market_data::ServerMsg::decode(&bin[..]) {
                                match server_msg.msg_type {
                                    Some(market_data::server_msg::MsgType::LogonResult(res)) => {
                                        if res.result_code == 0 {
                                            info!("🎊 LOGON EXITOSO: Autenticado en CQG Chicago.");

                                            // Suscripción al Futuro del Euro (6E)
                                            let sub_msg = market_data::ClientMsg {
                                                msg_type: Some(
                                                    market_data::client_msg::MsgType::Subscription(
                                                        market_data::MarketDataSubscription {
                                                            request_id: 1,
                                                            symbol: "6E".to_string(),
                                                            subscribe: true,
                                                        },
                                                    ),
                                                ),
                                            };
                                            let mut sub_buf = Vec::new();
                                            if let Ok(_) = sub_msg.encode(&mut sub_buf) {
                                                let _ = ws_stream
                                                    .send(Message::Binary(sub_buf.into()))
                                                    .await;
                                                info!("📡 Suscripción enviada para el símbolo: 6E");
                                            }
                                        } else {
                                            error!("❌ ERROR DE LOGON: {}", res.error_description);
                                        }
                                    }
                                    Some(market_data::server_msg::MsgType::MarketData(data)) => {
                                        // Aquí recibimos los trades reales de Chicago
                                        info!(
                                            "🎯 DATA 6E -> Trades: {} | Quotes: {}",
                                            data.trades.len(),
                                            data.quotes.len()
                                        );

                                        for trade in data.trades {
                                            info!(
                                                "🔥 [CHICAGO TRADE] P: {:.5} V: {}",
                                                trade.price, trade.volume
                                            );
                                        }
                                    }
                                    _ => {} // Otros mensajes como Heartbeats o Time del servidor
                                }
                            }
                        }
                        Ok(Message::Ping(p)) => {
                            // Responder a Pings del servidor para evitar desconexiones
                            let _ = ws_stream.send(Message::Pong(p)).await;
                        }
                        Ok(Message::Close(_)) => {
                            warn!("⚠️ El servidor de CQG cerró la conexión. Reiniciando...");
                            break;
                        }
                        Err(e) => {
                            error!("❌ Error en el flujo de CQG: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                error!("❌ No se pudo conectar a CQG: {}. Reintentando en 5s...", e);
            }
        }

        // Espera de seguridad antes de reintentar la conexión
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

