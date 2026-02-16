# 📜 FIX Bayesian Motor - Documentación Técnica (Legacy v1.4.0)

Este repositorio contiene la arquitectura de un motor de trading de alta frecuencia basado en **Inferencia Bayesiana**, desarrollado modularmente mediante ramas de funcionalidades (`feat/`) integradas en `develop`.

---

## 🏗 Fase 1: Infraestructura y Conectividad FIX
**Ramas:** `feat/f1-01` a `feat/f1-05`
* **Scaffold & Logger:** Configuración del entorno Rust y trazabilidad de logs de alta frecuencia.
* **TCP Async:** Capa de red con `Tokio` para flujos asíncronos de lectura/escritura.
* **FEFIX Engine:** Motor de diccionarios FIX 4.4 adaptado para cTrader/IC Markets.
* **Logon Sequence:** Validación del mensaje de sesión (`35=A`) y confirmación de autenticación.

---

## 📡 Fase 2: Ingesta de Datos y Gestión de Estado
**Ramas:** `feat/f2-01` a `feat/f2-05`
* **Session Health:** `Heartbeat` automático para mantenimiento de sesión activa.
* **Market Data:** Suscripción a flujos `Snapshot` e `Incremental Refresh`.
* **LOB Parser:** Extracción y procesamiento de niveles de profundidad del libro.
* **OrderBook Engine:** Gestión de Bids/Asks escalados a `i64` para precisión matemática.

---

## 🧠 Fase 3: Inteligencia Bayesiana y Feature Engineering
**Ramas:** `feat/f3-01` a `feat/f3-05`
* **Feature Engineering:** Cálculo de Imbalance, Velocidad de Ticks e Intensidad del LOB.
* **Uncertainty Filter:** Filtro de Incertidumbre Gaussiana para medición de ruido.
* **Bayesian Causal Network:** Evaluación probabilística del contexto macro.
* **Neural Brain:** Red neuronal bayesiana con reporte de predicción ($\mu$) e incertidumbre ($\sigma$).

---

## 🛡 Fase 4: Ejecución, Riesgo y Salida (Estabilización)
**Ramas:** `feat/f4-01` a `feat/f4-04`
* **Order Factory:** Generador institucional de `ClOrdID`.
* **Risk Manager:** Normalización de volumen (múltiplos de 1000) y redondeo de precisión.
* **Executor Tracker:** Validación de identidad de órdenes por ID y Side.
* **Exit Strategy:** Gestión local y silenciosa de TP/SL por cuantiles.

---

## 💡 Lecciones Aprendidas y Evolución (Hacia v2.0)

El paso a la versión **v2** nace de un cambio de paradigma tras identificar necesidades críticas en la operativa real:

1.  **Prioridad de Medición:** Se identifica que antes de ejecutar estrategias bayesianas, es vital realizar una **medición rigurosa de latencia y decisión**. La v2 implementará un modelo de tres capas:
    * **Capa 1 — Engineering Implementation Layer:** Infraestructura (FIX, telemetría, logging, resiliencia).
    * **Capa 2 — Execution Research Layer:** Análisis de latencia, slippage, spreads y ventanas operables.
    * **Capa 3 — Trading Strategy Layer:** Modelos bayesianos, sizing y reglas operativas basadas en los outputs de la Capa 2.

2.  **Arquitectura de Procesos:** Para mejorar la organización de ideas e información, se implementará la **Metodología SCRUM** como arquitectura de procesos.

3.  **Estrategia de Ramificación:** La organización simple de ramas `feat` evoluciona a una estrategia de **Gitflow** formal para un ciclo de vida de software más robusto.

4.  **Estructura Documental:** Se desarrollará una mejor organización de directorios, incluyendo una carpeta específica para la **gestión documental** del proyecto.

---

## 📑 Gestión de Versiones
* **Legacy Stable:** **`v1.4.0`** (Rama `develop`).
* **Próximo Ciclo:** **`v2.1.0`** (Enfoque en capas de infraestructura y telemetría).
