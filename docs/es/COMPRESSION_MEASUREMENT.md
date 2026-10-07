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

Para las columnas text/binary seleccionadas, el Nivel 2 también puede muestrear esa columna individualmente. Esto proporciona una tasa de compresión por columna en lugar de solo promedios a nivel de tabla.

Las relaciones de tablas en bases de datos en vivo utilizan una secuencia neutral de grupos de 1,000 filas con un descriptor por columna, longitudes de valor de ancho fijo y cargas útiles contiguas a las columnas. Esto mide la estructura relevante para la compresión sin capturar ningún protocolo de base de datos o de transferencia. Las relaciones por columna conservan `blueprint-compression-probe-v2`, cuyos valores con prefijo de longitud etiquetada siguen siendo la entrada de entropía más específica.

Los bloques de tablas de PostgreSQL utilizan `blueprint-columnar-transfer-probe-v2`, que pasa grupos de filas a través de un contexto zstd persistente de nivel 3 y se vacía después de cada grupo. MySQL y SQL Server utilizan `blueprint-columnar-transfer-probe-v3`: los mismos bytes neutros y un contexto persistente, con flujos adicionales en los límites de bloques de prueba de 256 KiB. Las muestras `nvarchar`, `nchar` y `ntext` de SQL Server se miden como distribuciones de bytes UTF-16LE. Las muestras `varchar`, `char` y `text` de SQL Server conservan su ancho de byte estrecho muestreado; el Blueprint registra el código de página del catálogo de la intercalación de origen como `utf-8`, `windows-N` o `code-page-N`. El controlador de la base de datos aún expone cadenas decodificadas al muestreador, por lo que no se afirma la identidad de bytes de la página de código heredada. La tabla `ratio_stddev` se mide a través de las salidas de grupos de filas externos. Los bloques de proyección por columna permanecen como mediciones de entropía independientes de un solo disparo y emiten `0.0`. Los Blueprints de versiones anteriores pueden contener `blueprint-columnar-transfer-probe-v1`; las relaciones con etiquetas diferentes no son comparables.

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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
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

Estos valores se utilizan para estimar el tamaño de la transferencia de red.

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
muestra estadística aleatoria. Las ventanas de rango no finales tienen un límite
superior exclusivo. Las regiones de clave primaria dispersas o sesgadas pueden
dejar una ventana incompleta, pero una ventana no puede volver a leer filas de la
siguiente. Otros métodos alternativos menos idóneos también se registran mediante
`sampled_with_bias` y `bias_reason`.

Blueprint registra la estructura de una muestra delimitada por separado de esos campos de texto. El muestreo de rangos de claves primarias numéricas en MySQL emite `sample_layout = "primary-key-range-windows"` y ordena cada ventana según la clave primaria completa.

Las muestras sesgadas siguen siendo útiles, pero tienen una menor fiabilidad. El registro de auditoría indica que se habilitó el muestreo de filas y el recuento de bytes de la sonda codificada localmente. Los totales de bytes de la sesión de la base de datos se informan como `unknown` cuando el controlador no los expone.

## Configuración práctica del muestreo

Primera pasada segura para producción:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Medición más precisa cuando se dispone de una réplica de lectura o una ventana de mantenimiento:

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

## ¿Cómo se interpretan las mediciones?

El campo `sample_encoding` forma parte del contrato. Las ratios solo son comparables dentro de una misma etiqueta de codificación, porque diferentes codificaciones de muestra pueden producir diferentes ratios de compresión para los mismos datos lógicos. En particular, la ratio de transferencia columnar a nivel de tabla y las ratios v2 por columna son mediciones complementarias y no deben sustituirse entre sí.
