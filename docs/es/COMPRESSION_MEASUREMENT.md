# Medición de compresión

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../COMPRESSION_MEASUREMENT.md).

**Idiomas:** [English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | [Français](../fr/COMPRESSION_MEASUREMENT.md) | **Español** | [Polski](../pl/COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | [中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` puede medir opcionalmente el grado de compresión de datos
representativos de las tablas. Esto mejora la precisión de las estimaciones de
DBWarp porque el tiempo de transferencia por WAN y el coste del tráfico saliente dependen
de los bytes comprimidos, no del tamaño bruto de las tablas.

La medición de compresión es opcional y requiere consentimiento explícito. Una ejecución en vivo interactiva puede aceptar la confirmación previa; las ejecuciones desatendidas y de archivos estructurados usan:

```bash
--measure-compression --yes
```

Sin esas opciones, la herramienta solo lee metadatos del catálogo.

## Qué se muestrea

Para cada tabla de usuario, la herramienta lee en memoria un número acotado de
filas, las codifica en un búfer determinista de tramas de fila, comprime ese
búfer localmente con zstd de nivel 3, registra proporciones
redondeadas y descarta el búfer.

Para determinadas columnas de texto o binarias, el nivel 2 también puede
muestrear únicamente esa columna. Esto permite que las herramientas de
planificación posteriores reproduzcan la entropía de cada columna en lugar de
basarse solo en promedios por tabla.

Cada medición es una trama zstd independiente de una sola pasada con el tamaño de entrada declarado. La varianza de las proporciones (`ratio_stddev`) se mide sobre fragmentos de 64 KiB alineados a filas del mismo búfer, de modo que describe la transferencia que predice el estimador en lugar de un único promedio del búfer completo. Como el tamaño de entrada se declara, zstd selecciona parámetros adaptados al tamaño coherentes con la forma en que el estimador modela la transferencia. En muestras pequeñas (por debajo de aproximadamente 1 MiB) esto puede desplazar notablemente las proporciones frente a capturas de versiones anteriores que medían con un contexto de streaming sin tamaño declarado; las proporciones de tablas pequeñas no son directamente comparables a través de ese límite. La medición con tamaño declarado es la que coincide con la transferencia.

Los bytes muestreados no se escriben en disco, no se incluyen en `blueprint.toml`,
no se incluyen en el registro de auditoría y no se envían a ningún lugar salvo
desde el servidor de base de datos al proceso local que usted ejecutó.

## Concurrencia de workers locales

El muestreo de la base de datos siempre utiliza una sola conexión secuencial.
La opción `--compression-workers N` solo paraleliza la compresión local de
muestras ya leídas en memoria. Acepta de 1 a 32 workers y usa 1 de forma
predeterminada para minimizar el impacto en el host de origen. Auméntelo
explícitamente para utilizar más CPU local:

```bash
--measure-compression --yes \
--compression-workers 4
```

Los valores superiores pueden reducir el tiempo transcurrido cuando zstd es el
cuello de botella, pero aumentan el uso local de CPU y la memoria máxima. No
crean conexiones simultáneas de muestreo. Cada worker posee sus contextos zstd
y la cola de entrada está limitada al número de workers. El orden de salida y
los valores del Blueprint v6 siguen siendo deterministas.

El recopilador evita consultas de filas y estilo solo cuando un valor de
catálogo mantenido por el motor demuestra con seguridad que una tabla estaba
vacía al leer el catálogo. PostgreSQL exige estadísticas analizadas recientes
sin modificaciones posteriores; SQL Server utiliza su contador de filas de
partición. Las estimaciones de filas de MySQL pueden indicar cero para una
tabla no vacía, por lo que no se usan para omitir el muestreo. Esta diferencia
conservadora protege la fidelidad.

## Qué aparece en el archivo Blueprint

Solo se emiten cifras de resumen. Para columnas similares a texto, la pasada de
nivel 2 puede emitir una etiqueta de estilo acotada como `json`, `xml`,
`natural-text`, `base64`, `hex`, `numeric-text` o `mixed`.

Ejemplo:

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

Estos valores ayudan a las herramientas posteriores aprobadas a estimar el
tamaño de la transferencia de red y a generar datos sintéticos de texto o
binarios con una capacidad de compresión similar.

## Por qué importa

Dos bases de datos con el mismo tamaño bruto de tablas pueden comportarse de
forma muy distinta durante una migración:

- JSON, XML, códigos empresariales repetidos, texto disperso y texto en lenguaje
  natural suelen comprimirse bien.
- Los valores cifrados, blobs ya comprimidos, tokens aleatorios y datos binarios
  de alta entropía no se comprimen bien.
- Los datos `nvarchar` de SQL Server presentan una distribución de bytes
  diferente a la del texto UTF-8 y se codifican de forma acorde para el
  muestreo.

Una pequeña medición local suele resultar más útil que inferir a partir de los
tipos de columna.

## Sesgo y transparencia

Algunos motores no ofrecen un muestreo de tablas perfectamente uniforme. Cuando
la herramienta recurre a un método menos idóneo, el archivo Blueprint lo indica
mediante `sampled_with_bias` y `bias_reason`.

Las muestras sesgadas siguen siendo útiles, pero las herramientas posteriores
deberían tratarlas con menor confianza. El registro de auditoría deja constancia de
que se habilitó el muestreo y de los bytes row-frame codificados localmente. Los
bytes de red se indican como `unknown` cuando el controlador no los expone.

## Configuración práctica del muestreo

Primera pasada segura para producción:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Mejor entrada para el estimador cuando se dispone de una réplica de lectura o
una ventana de mantenimiento:

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

Las bases de datos grandes no requieren muestras enormes. El objetivo es una
señal de compresión estable, no un perfilado exacto de cada fila.
`--max-wall-secs` es un plazo estricto para toda la captura en vivo, incluida la
conexión, los catálogos, RTT y el muestreo; no se reinicia en cada fase.

El muestreo de bases de datos en vivo también tiene un límite no configurable de
16 MiB de carga proyectada por tabla. La proyección SQL trunca en el servidor
las celdas de anchura variable y reduce el límite de filas para tablas
excepcionalmente anchas antes de que el controlador reciba los datos. Por tanto,
los valores LOB muy grandes aportan prefijos acotados en lugar de todo su
contenido. La auditoría registra el límite activo de carga por tabla y el total
exacto de bytes de tramas de filas codificados localmente.

## Cómo la utilizan los consumidores posteriores

Un consumidor posterior debe utilizar la evidencia de compresión en este orden:

1. bloques de compresión por columna reconocidos;
2. bloques de compresión por tabla reconocidos;
3. valores predeterminados de tipo y estilo cuando no existe una proporción
   medida.

El campo `sample_encoding` forma parte del contrato. Los consumidores solo
deberían utilizar proporciones con una etiqueta de codificación reconocida, porque
codificaciones de muestra distintas pueden producir proporciones de compresión
diferentes para los mismos datos lógicos.
