# Métricas de "Calidad de Tesis"
Dado que no te preocupa la velocidad, las métricas que deberíamos observar son de precisión, no de tiempo:
1. Max Adverse Excursion (MAE) (Drawdown): Registra el current_mid más alto (peor para un SELL) durante la vida del trade. 
2. Tiempo de Retención (Holding Time): ¿Cuánto tarda en promedio en tocar el profit? Esto te dirá si el modelo está capturando micro-tendencias o ruidos. Resta Local::now() menos pos.opened_at
3. Probabilidad Real vs Estimada. Compara si los trades con >80% de probability realmente llegan al TP. Compara la initial_probability de los trades ganadores. Si todos los ganadores tenían >85%, quizás debas subir tu filtro de entrada del 80% al 85%.
4. Confidence Decay (Deriva): Registraremos el valor más bajo de probabilidad que el cerebro emitió mientras el trade estaba abierto. Si entras con 80% de probabilidad y a los 5 minutos baja a 40% (pero el precio aún no toca el TP), el modelo está viendo algo nuevo que no le gusta. Esta métrica te servirá en el futuro para programar una "Salida de Emergencia" si la confianza se desploma.

# Métricas de Hardware (CPU y Memoria)
## Consumo de CPU
Observa en tu monitor de sistema (o con htop) cuánto porcentaje de CPU consume tu proceso de Rust durante el entrenamiento.
* Métrica de Estrés: Observa el CPU Usage del proceso cuando el mercado esté muy rápido. Si el motor usa >12.5% de CPU: Significa que está saturando un núcleo completo de tu PC (1 de 8). En un VPS Silver (2 núcleos), esto representaría el 50% de la capacidad total.

## Consumo de memoria
* Memory Footprint: Monitorear el consumo de RAM para asegurar que no haya fugas de memoria en las colas de datos.
Ver si lo siguiente esta creciendo sin control. 
* VecDeque 
* FeatureCollector

## Mejoras con 'Nanosecond Rust'
1. Sustituir Inferencia por Tablas: Pre-calcular la lógica para no hacer Monte Carlo en vivo.
2. Pinned CPU: Asegurar que el motor corra en un núcleo dedicado del VPS.
3. Static Allocation: Eliminar los Vec y String dinámicos en el loop principal.

# Persistencia de Pesos (Brain)
Para que el modelo no sea un "eterno principiante" cada vez que lo abras en tu PC local:<br>
Archivo: state_engine_brain.bin.<br>
Lógica: * Al arrancar, el Brain intenta cargar los pesos. Si no existen, inicializa con valores aleatorios.

Cada vez que se cierra un trade, el sistema guarda el estado actual de los pesos y las varianzas en el disco.

Impacto: Esto reduce el overfitting a corto plazo, ya que el modelo acumula experiencia de diferentes días y horas de mercado, creando una base de conocimiento más robusta.

# Hoja de Ruta de Archivos a Modificar
Para ejecutar este plan, modificaremos los archivos en este orden:

1. Cargo.toml: Añadiremos serde, bincode (para los pesos) y sysinfo (para el hardware).

2. src/brain.rs: Añadiremos las funciones de serialización para guardar/cargar la red neuronal.

3. src/state.rs: Añadiremos los campos de métricas a la estructura Position.

4. src/executor.rs: Implementaremos la lógica de seguimiento del MAE y la Deriva de Confianza.

5. src/main.rs: Orquestaremos la carga de datos al inicio y el reporte de sistema.
