#  Referencia: Conexión Live LOB-Only (v1.0.0)

##  Estado del Proyecto
Esta rama representa el **punto de referencia estable** de la conexión con IC Markets (cTrader FIX API) en cuenta Real. Se ha consolidado como una base funcional para el procesamiento de datos de mercado de nivel 2 (Market Depth).

**Fecha de referencia:** 16 de Abril de 2026
**Entorno:** Producción (Live)
**Activo principal:** EURUSD (Symbol ID: 1)

##  Funcionalidades Alcanzadas
- **Conexión FIX Estable:** Handshake, Heartbeat y Logon exitosos con el servidor de cotizaciones (`QUOTE`).
- **Suscripción Full LOB:** Solicitud `MarketDataRequest (35=V)` configurada para profundidad total (`264=0`).
- **Procesamiento Incremental:** Manejo eficiente de mensajes `MarketDataIncrementalRefresh (35=X)`.
- **Detección de Muros:** Capacidad para identificar liquidez institucional (bloques de 20M+ unidades/200 lotes).
- **Gestión de Sesión:** Manejo de números de secuencia y persistencia de sesión FIX.

##  Limitaciones Identificadas (Motivo de esta Referencia)
1. **Ausencia de Tag 269=2 (Trades):** El feed actual de IC Markets no proporciona ejecuciones públicas (Time & Sales) a pesar de solicitarse explícitamente en el mensaje `35=V`.
2. **Dependencia de LOB:** La estrategia de absorción actual se basa exclusivamente en cambios de volumen (`271`) y cancelaciones (`279=2`).
3. **Falta de Confirmación de Agresividad:** Sin el `269=2`, no es posible validar si un muro fue consumido o cancelado (spoofing).

##  Estructura Técnica
- **Puerto de Datos:** 5201 (Quotes)
- **Protocolo:** FIX 4.4
- **Lenguaje:** Rust (Tokio para asincronía)

##  Próximas Líneas de Investigación
A partir de este punto, el desarrollo se divide en:
1. **`research/external-feeds`**: Integración con Binance/OANDA para obtener el flujo de trades (`269=2`).
2. **`research/lob-optimization`**: Desarrollo de "Virtual Tape" para inferir ejecuciones mediante deltas de volumen en el libro.

---
*Este es un punto de control inmutable. No realizar cambios directamente en esta rama.*
