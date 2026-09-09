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

Cuando la medición de compresión está deshabilitada, la captura de bases de
datos en vivo no muestrea valores de filas de tablas de usuario. El
comportamiento con archivos estructurados es distinto: los registros Avro
deben recorrerse igualmente para recopilar recuentos de filas, longitudes y
metadatos de nulos; consulte [Archivos estructurados](STRUCTURED_FILES.md).

## Qué se muestrea

Para cada tabla de usuario apta cuya ausencia de filas no se haya demostrado
con seguridad, la herramienta lee en memoria un número acotado de filas, las
codifica en búferes de sonda transitorios estables, comprime esos búferes
localmente con zstd de nivel 3 y deriva mediciones agregadas de compresión, densidad de
NULL, cardinalidad/frecuencia, longitud y estilo antes de descartar los valores
muestreados y las huellas temporales.

Para determinadas columnas de texto o binarias, el nivel 2 también puede
muestrear únicamente esa columna. Esto permite que las herramientas de
planificación posteriores reproduzcan la entropía de cada columna en lugar de
basarse solo en promedios por tabla.

Las proporciones de tablas de bases de datos en vivo utilizan una secuencia
neutra de grupos acotados de 1000 filas, con un descriptor por columna,
longitudes de valor de anchura fija y cargas contiguas por columna. Esto mide la
estructura relevante para la compresión que comparten los transportes masivos,
sin capturar un protocolo de base de datos ni un formato de red de DBWarp. Las
proporciones por columna conservan `blueprint-compression-probe-v2`, cuyos
valores etiquetados y prefijados por longitud siguen siendo la entrada de
entropía más específica.

Los bloques de tabla de PostgreSQL usan actualmente
`blueprint-columnar-transfer-probe-v2`, que pasa los grupos de filas por un
contexto persistente de zstd de nivel 3 y lo vacía tras cada grupo. MySQL y SQL
Server usan `blueprint-columnar-transfer-probe-v3`: los mismos bytes neutros y
el mismo contexto persistente, con vaciados adicionales en los límites de
bloques de sonda de 256 KiB. Las muestras `nvarchar`, `nchar` y `ntext` de SQL
Server se miden como distribuciones de bytes UTF-16LE. `varchar`, `char` y
`text` conservan la anchura estrecha muestreada;
el Blueprint registra la página de códigos de la intercalación de origen como
`utf-8`, `windows-N` o `code-page-N`, para que un consumidor aprobado pueda
elegir un codificador nativo compatible en vez de ensanchar los valores. El
controlador sigue entregando cadenas decodificadas al muestreador, por lo que
no se afirma identidad de bytes para páginas de códigos heredadas. El
`ratio_stddev` de la tabla se mide entre las salidas de los grupos de filas
externos. Los bloques de proyección por columna siguen siendo mediciones de
entropía independientes de una sola pasada y emiten `0.0`. Las mediciones de
tabla anteriores etiquetadas `blueprint-columnar-transfer-probe-v1` usaban una
operación con tamaño declarado sobre las tramas unidas; la versión explícita
impide que esas proporciones se reinterpreten de forma silenciosa con la
política de streaming actual.

Los bytes muestreados solo viajan por la sesión de base de datos seleccionada
hasta el proceso local. No se escriben en disco, no se incluyen en
`blueprint.toml` ni en el registro de auditoría, no se cargan ni se envían a la
infraestructura de DBWarp.

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
y la cola de entrada está limitada al número de workers. El número de workers
no cambia las mediciones. El orden de las etiquetas anónimas varía
intencionadamente con la clave nueva predeterminada; reutilice un archivo
protegido `--anonymization-key-file` solo para comparaciones aprobadas entre
ejecuciones.

El recopilador evita consultas de filas y estilo solo cuando un valor de
catálogo mantenido por el motor demuestra con seguridad que una tabla estaba
vacía al leer el catálogo. PostgreSQL exige estadísticas analizadas recientes
sin modificaciones posteriores; SQL Server utiliza su contador de filas de
partición. Las estimaciones de filas de MySQL pueden indicar cero para una
tabla no vacía, por lo que no se usan para omitir el muestreo. Esta diferencia
conservadora protege la fidelidad.

## Qué aparece en el archivo Blueprint

Solo se emiten resúmenes agregados. Para columnas similares a texto, la pasada de
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
bias_reason = "unordered_limit_after_empty_TABLESAMPLE"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
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
- El texto Unicode y el texto estrecho de SQL Server tienen distribuciones de
  bytes distintas. El muestreador modela `nvarchar` como UTF-16LE y registra la
  página de códigos de intercalación necesaria para interpretar `varchar`, sin
  tratar todas las columnas de texto como UTF-8.

Una pequeña medición local suele resultar más útil que inferir a partir de los
tipos de columna.

## Sesgo y transparencia

Algunos motores no ofrecen un muestreo de tablas perfectamente uniforme. MySQL
distribuye una muestra acotada entre cuatro rangos de clave primaria numérica
cuando esa ruta de acceso está disponible; en caso contrario recurre a `LIMIT N`.
Ambos se marcan explícitamente como sesgados porque ninguno constituye una
muestra estadística aleatoria. Otros métodos alternativos menos idóneos también
se registran mediante `sampled_with_bias` y `bias_reason`.

Cuando una muestra acotada tiene una disposición que afecta a la generación
sintética, Blueprint la registra por separado de esos campos de texto. El
muestreo de rangos de clave primaria numérica de MySQL emite
`sample_layout = "primary-key-range-windows"` y ordena cada ventana por la clave
primaria completa. Esto permite conservar la localidad agrupada de claves
compuestas sin analizar `sample_method` ni `bias_reason`.

Las muestras sesgadas siguen siendo útiles, pero las herramientas posteriores
deberían tratarlas con menor confianza. El registro de auditoría deja constancia de
que se habilitó el muestreo y de los bytes de sonda codificados localmente. Los
bytes de la sesión de base de datos se indican como `unknown` cuando el controlador no los expone.

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
16 MiB de carga proyectada por tabla. La proyección SQL inicial se presupuesta
por tipo y observa por separado las longitudes originales en octetos. Cuando
se ha reducido un valor proyectado, MySQL y SQL Server pueden reintentar con
menos filas y límites por columna revisados que sigan respetando el presupuesto.
Los valores demasiado anchos para ese presupuesto siguen siendo prefijos
acotados; la información de procedencia de compresión y de resúmenes de valores
registra esta limitación, mientras que las estadísticas de longitud conservan
las longitudes originales de los valores muestreados comunicadas por el servidor,
bajo la política de fidelidad de longitud seleccionada.

El límite no se aplica a los bytes de red ni a la memoria del proceso. La
codificación del protocolo, los metadatos de longitud original, los reintentos y
los búferes del controlador añaden sobrecarga. La auditoría registra el límite
de carga configurado, las consultas realizadas y el total exacto de bytes de
sonda codificados localmente; no informa de tráfico medido en la conexión de
base de datos.

## Cómo la utilizan los consumidores posteriores

Un consumidor posterior debe utilizar la evidencia de compresión en este orden:

1. bloques de compresión por columna reconocidos;
2. bloques de compresión por tabla reconocidos;
3. valores predeterminados de tipo y estilo cuando no existe una proporción
   medida.

El campo `sample_encoding` forma parte del contrato. Los consumidores solo
deberían utilizar proporciones con una etiqueta de codificación reconocida, porque
codificaciones de muestra distintas pueden producir proporciones de compresión
diferentes para los mismos datos lógicos. En particular, la proporción de la
sonda de transferencia columnar de tabla y las proporciones v2 por columna son
mediciones complementarias y no deben sustituirse entre sí.
