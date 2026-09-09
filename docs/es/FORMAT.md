# Formato de archivo v6 de DBWarp Blueprint

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../../FORMAT.md).

**Idiomas:** [English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | **Español** | [Polski](../pl/FORMAT.md) | [日本語](../ja/FORMAT.md) | [中文](../zh/FORMAT.md)

Legible para personas. Fácil de comparar. Revisable forensemente.

> **Este formato reduce el riesgo de canales encubiertos y de divulgación directa
> mediante un esquema acotado, identificadores basados en una clave secreta y precisión numérica
> documentada. La estructura anónima del grafo y los campos exactos opcionales
> aún pueden identificar una carga de trabajo, por lo que debe revisar el archivo
> conforme a su propia política de clasificación de datos.**

## Cabecera del archivo

Literal, byte a byte:

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

La línea en blanco forma parte de la cabecera canónica. El recopilador Rust
emite exactamente esta cabecera y ningún otro comentario. El normalizador de la
ruta SQL alternativa la conserva literalmente y después añade un comentario
fijo `Producer: blueprint_format.py SQL fallback` con el origen de la clave,
para que los destinatarios puedan distinguir el productor. Esto no afirma que
los demás campos estructurados no puedan identificar un esquema o un grafo de
dependencias distintivo.

## Campos de nivel superior

| Campo | Tipo | Descripción |
|---|---|---|
| `schema_version` | int | Versión del formato. Actualmente `6`; las versiones 1 a 5 siguen siendo legibles. |
| `generated_at` | ISO-8601 string | Marca de tiempo UTC, con resolución de segundos y sin fracción. **Se puede fijar** mediante la opción CLI `--generated-at "2026-04-26T00:00:00Z"`. Las capturas en vivo idénticas byte a byte también requieren el mismo `--anonymization-key-file` protegido, estado de origen, opciones y productor. El registro de auditoría incluye `generated_at_pin: ...` cuando se establece la opción, de modo que la fijación sea visible forensemente. Ninguna variable de entorno fija este valor. |
| `engine` | string | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"parquet"` o `"avro"`. |
| `engine_version` | string | Versión numérica restringida del producto utilizada para la captura y generación de bases de datos en vivo según su versión; vacía para fuentes de archivos estructurados. Se excluyen los banners del productor o la distribución. |
| `source_kind` | string | Una de `"production"`, `"staging"`, `"scrubbed-replica"`, `"synthetic"`. Declarada por el cliente. |
| `length_metadata` | string | Marcador de compatibilidad heredada: `"hybrid-v2"`, `"exact"`, `"rounded"` o `"not-captured"`. Los consumidores nuevos deben utilizar los tres campos siguientes. |
| `declared_length_fidelity` | string | `"exact"` para las capacidades de caracteres declaradas de PostgreSQL y para los modos MySQL equilibrado predeterminado y exacto; `"coarse-rounded-v1"` para privacidad estricta de MySQL; `"not-captured"` cuando no esté disponible. |
| `index_length_fidelity` | string | `"exact"` para prefijos de índice MySQL equilibrados predeterminados o exactos; `"rounded-down-v1"` para privacidad estricta; `"not-captured"` cuando no esté disponible. |
| `observed_length_fidelity` | string | `"relative-rounded-v2"` de forma predeterminada cuando se muestrea, `"exact"` en modo exacto, `"coarse-rounded-v1"` en modo estricto o `"not-sampled"`. La cobertura de muestreo sigue siendo un requisito independiente por columna. |
| `[totals]` | inline table | Recuentos agregados (consulte más abajo). |
| `[network]` | table | Evidencia opcional de la conexión cliente-base de datos y del RTT de consulta. |
| `[database_topology]` | table | Obligatorio para fuentes de base de datos con esquema v6. Despliegue, rol local, visibilidad y evidencia de catálogo acotados y sin nombres. Ausente para archivos estructurados. |
| `[dataset_scope]` | table | Obligatorio para todo Blueprint con esquema v6. Declara qué cubren los totales y si la cobertura de tablas, filas y bytes es completa. |
| `[tables.X]` | tables | Uno por tabla, con identificador anonimizado. |
| `[fk_edges]` | inline table | Grafo de claves foráneas entre tablas anonimizadas. Opcional. |
| `[artifact_inventory]` | table | Recuentos acotados y sin nombres de objetos no tabulares, grafo de dependencias anónimo opcional, requisitos externos y censo acotado opcional de características del lenguaje de programación. Solo para orígenes de bases de datos. |

## `[totals]`

| Campo | Tipo | Precisión |
|---|---|---|
| `table_count` | int | exacta |
| `row_count` | int | suma de los valores `rows` redondeados de cada tabla |
| `table_bytes` | int | suma de los valores `table_bytes` redondeados de cada tabla |
| `index_bytes` | int | suma de los valores `index_bytes` redondeados de cada tabla |

Estas cifras no son automáticamente totales de todo el clúster. Siempre deben
interpretarse junto con `[dataset_scope]`. Una pasarela o coordinador con
fragmentación puede mostrar un catálogo aparentemente completo sin alojar los
fragmentos subyacentes. El esquema v6 representa explícitamente esa
incertidumbre en vez de tratar silenciosamente las estadísticas locales como
verdad global.

## `[database_topology]` (fuentes de base de datos con esquema v6)

Este bloque registra solo hechos acotados visibles a través del endpoint de
base de datos conectado. Nunca almacena nombres de nodos o hosts, direcciones
IP, nombres de clúster o canal de replicación, identificadores de servidor ni
endpoints.

| Campo | Valores / regla |
|---|---|
| `contract` | Siempre `dbwarp-blueprint-topology/v1`. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` o `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member` o `unknown`. |
| `visibility` | `full`, `partial` o `unknown`; describe la evidencia de topología, no la corrección de los datos. |
| `member_count` | Número de miembros visibles mediante consultas de evidencia correctas. `0` significa desconocido, nunca cero miembros. |
| `identifiers_redacted` | Debe ser `true`. |
| `role_counts` | Recuentos opcionales por token cerrado de rol. La visibilidad completa exige que sumen `member_count`. |
| `features` | Tokens cerrados y ordenados como `citus`, `mysql-group-replication`, `mysql-galera`, `mysql-ndb`, `postgresql-streaming-replication`, `sqlserver-availability-group` o `vitess`. |
| `catalogs_read` | Etiquetas cerradas y ordenadas de catálogos de topología leídos correctamente. |
| `catalogs_unreadable` | Etiquetas cerradas y ordenadas de catálogos de topología no legibles. Cualquier entrada impide afirmar visibilidad completa. |

Un endpoint ordinario puede indicar legítimamente
`deployment = "unknown"` y aun así proporcionar estadísticas locales
completas de una copia íntegra. Blueprint no deduce que un servidor corriente
sea `single-node` solo porque no se observó ninguna función de clúster.

## `[dataset_scope]` (esquema v6)

Este bloque califica independientemente cada total de dimensionamiento. Los
consumidores deben rechazar la aritmética no calificada del conjunto completo
cuando cualquier dimensión necesaria sea `incomplete` o `unknown`.

| Campo | Valores / regla |
|---|---|
| `contract` | Siempre `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` o `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` o `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` o `unknown`. |
| `size_completeness` | `complete`, `incomplete` o `unknown`. |
| `row_count_method` | Token cerrado de procedencia como `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter` o `distributed-aggregate`. |
| `size_method` | Token cerrado de procedencia como `postgres-local-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `citus-distributed-relation-size` o `distributed-aggregate`. |
| `limitations` | Motivos cerrados y ordenados de cobertura incompleta o desconocida. Se requiere al menos uno salvo que todas las dimensiones estén completas. |

`selection-limited` significa que los totales y las declaraciones de integridad cubren exactamente los esquemas solicitados mediante el selector repetible en vivo `--schema`; no afirman cubrir toda la base de datos conectada. Si se omite `--schema`, se conserva la captura de todos los esquemas visibles.

Los recopiladores nativos de PostgreSQL, MySQL y SQL Server consultan los
catálogos de topología compatibles antes de decidir si las estadísticas
locales pueden representar el conjunto lógico. Las pasarelas distribuidas
conocidas suprimen totales inseguros cuando no existe un agregado fiable. El
formateador SQL alternativo no dispone de sonda de topología, por lo que emite
sus estimaciones locales útiles con todas las dimensiones marcadas como
`unknown` y las limitaciones `topology-unobserved` y
`topology-visibility-unknown`.

Los Blueprints estructurados de Parquet y Avro omiten
`[database_topology]` y usan `layout = "structured-dataset"` con procedencia
del footer o contenedor.

Blueprint no ejecuta una prueba de velocidad de almacenamiento durante la
captura normal ni deduce el hardware del servidor de base de datos a partir de
la máquina cliente. Los totales de bytes describen el volumen almacenado según
el método de catálogo indicado; no afirman el tipo de disco, IOPS, caudal, CPU,
RAM ni rendimiento de la migración de destino.

## `[network]` (opcional)

Estadísticas observadas en el entorno del cliente sobre el tiempo de ida y
vuelta de la red desde la herramienta Blueprint hasta la base de datos de origen.
**NO** es el RTT entre el origen y el destino de la migración: únicamente
constituye evidencia de la distancia entre la herramienta Blueprint y la base de
datos de origen del cliente durante la ejecución. El estimador posterior solo
lo utiliza como comprobación de coherencia del RTT de migración proporcionado
por el operador (por ejemplo, afirmar un RTT de migración de 200 ms resulta
inverosímil si la sonda local del cliente fue de 0,4 ms; probablemente la
herramienta Blueprint se ejecutaba en el propio origen).

La sonda se ejecuta después de establecer la conexión y antes de consultar el
catálogo, por lo que los tiempos no quedan distorsionados por el calentamiento
de la caché de consultas. Ejecuta **5× `SELECT 1`** y emite la latencia mediana.
Cada `SELECT 1` devuelve la constante entera 1; esta sonda nunca lee datos de
filas.

El bloque no aparece cuando el cliente proporciona `--no-rtt-probe` o si la
propia sonda falla durante su ejecución (se registra como advertencia no fatal
en stderr y en el registro de auditoría; el archivo Blueprint sigue emitiéndose
sin el bloque).

| Campo | Tipo | Precisión |
|---|---|---|
| `sample_count` | int | exacta (siempre 5 en v1) |
| `connect_total_ms` | int | tiempo de reloj total desde el inicio de la conexión TCP hasta que la sesión autenticada está lista, en milisegundos. Incluye el protocolo de enlace TCP, el protocolo de enlace TLS (cuando corresponde) y el desafío/respuesta de autenticación. Redondeado al milisegundo más cercano. Normalmente equivale a 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | latencia mediana de una sola ida y vuelta de las 5 muestras `SELECT 1`, en milisegundos. Redondeada al milisegundo más cercano. El nivel natural de ruido de la red (≥ 1 ms en la práctica) es mayor que la granularidad del redondeo, por lo que se elimina cualquier canal encubierto de bits bajos sin perder precisión útil. Los valores de LAN inferiores a un milisegundo se reducen a 0 o 1. |
| `query_rtt_ms_p95` | int | percentil 95 de las 5 muestras calculado mediante el método del rango más próximo (la observación más lenta), en milisegundos. Redondeado al milisegundo más cercano. Úselo con p50 para detectar picos breves de latencia; cinco muestras solo sirven como orientación y no constituyen una prueba comparativa de una carga de trabajo. |

Las 5 consultas de la sonda aparecen en el registro de auditoría como una
**única entrada de resumen** (no como 5 filas independientes) con la etiqueta
`5x SELECT 1 (RTT probe; constant integer 1, no row data)`, en consonancia con
la postura de confianza de que no se lee contenido de filas.

## `[tables.<id>]`

El identificador es `table-NNN`, donde `NNN` es el ordinal indexado desde 1 en
un orden HMAC-SHA256 con separación por dominio del nombre de esquema y tabla.
La clave predeterminada se genera de nuevo para el proceso y nunca se emite. Si
se proporciona el mismo `--anonymization-key-file` custodiado por el cliente, se
conserva el orden entre ejecuciones de comparación aprobadas.

| Campo | Tipo | Precisión / valores |
|---|---|---|
| `rows` | int | redondeado: a la centena más cercana (≤10k), al millar (≤1M), a la decena de millar (>1M) |
| `table_bytes` | int | redondeado: al 1KiB, 1MiB o 100MiB más cercano según la magnitud |
| `index_bytes` | int | redondeado: igual que `table_bytes` |
| `schema` | string | identificador anonimizado `schema-A`, `schema-B`, ..., `schema-AA` |
| `kind` | string | Token cerrado opcional del esquema v6: `partitioned`, `materialized-view`, `temporal-current`, `temporal-history`, `memory-optimized`, `external`, `graph-node` o `graph-edge`. Se omite para una tabla ordinaria o cuando la evidencia es desconocida. |
| `unlogged` | bool | Observación opcional del catálogo de PostgreSQL en el esquema v6. Se omite si no se capturó; `false` explícito confirma una tabla registrada. |
| `partition_strategy` | string | Token opcional del esquema v6 para `partitioned`: `range`, `list`, `hash`, `key` o `linear-hash`. |
| `partition_count` | int | Número exacto y positivo de particiones hoja en el esquema v6, obligatorio con `kind = "partitioned"`. |
| `partition_key_cols` | array of int | Ordinales de una clave de partición simple en el esquema v6. Se omite para una clave de expresión o sin evidencia del catálogo; nunca se serializa la expresión. |
| `partition_rows_max` | int | Estimación redondeada opcional de las filas de la mayor partición hoja en el esquema v6. |
| `temporal_history` | string | Identificador de la tabla `temporal-history` asociada en el esquema v6, obligatorio para `temporal-current`. |
| `counted_in_totals` | bool | Esquema v6. La omisión incluye la tabla en todos los totales. `external` exige `false`, que la excluye de `table_count`, `row_count`, `table_bytes` e `index_bytes`; ningún otro valor explícito es canónico. |
| `check_count` | int | Número estructural exacto opcional de restricciones CHECK en el esquema v6. La omisión significa desconocido; `0` confirma que no hay ninguna. |
| `has_clustered_index` | bool | siempre `false` para PostgreSQL |
| `stats_freshness` | string | `"fresh"` / `"stale"` / `"never_analyzed"` para PostgreSQL, derivado de las marcas de tiempo de análisis de `pg_stat_all_tables`; vacío para MySQL, SQL Server, archivos estructurados y rutas de SQL alternativo que no pueden determinar la actualidad de las estadísticas sin permisos de acceso más amplios. MySQL `UPDATE_TIME` no se utiliza deliberadamente porque describe la modificación de los datos fuente, no cuándo se ejecutó `ANALYZE TABLE`. |
| `[tables.<id>.cols.<cid>]` | sub-tables | uno por columna |
| `[tables.<id>.idxs.<iid>]` | sub-tables | uno por índice |
| `[tables.<id>.compression]` | sub-table | solo si es de nivel 2 |

## `[tables.<id>.cols.<cid>]`

El identificador es `col-N`, donde `N` es el orden natural del atributo de la
columna (indexado desde 1, conservando el ordinal en disco). Se mantiene estable
entre ejecuciones.

| Campo | Tipo | Notas |
|---|---|---|
| `ordinal` | int | el mismo N que en el identificador |
| `type` | string | familia de tipos normalizada, como `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` o `"user-defined"`. No se emiten nombres reales de dominios, enumeraciones, alias, tipos compuestos ni tipos definidos por el usuario. |
| `nullable` | bool | |
| `value_source` | string | Token cerrado opcional del esquema v6: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` o `rowversion`. Se omite para un valor ordinario o evidencia desconocida. |
| `has_default` | bool | Observación opcional del catálogo en el esquema v6. La omisión significa desconocido; `false` confirma que no hay valor predeterminado. |
| `default_kind` | string | Clasificación opcional `constant`, `function` o `expression` en el esquema v6, válida solo con `has_default = true`. Nunca se serializan el texto ni los literales. |
| `type_kind` | string | Token cerrado opcional del esquema v6: `enum`, `set`, `domain`, `composite`, `array`, `range` o `alias`. Se omite para un tipo base o evidencia desconocida. |
| `member_count` | int | Número estructural exacto y positivo de miembros en el esquema v6, obligatorio solo para `enum` y `set`. Nunca se serializan sus nombres. |
| `domain_has_check` | bool | Observación opcional del CHECK de un dominio en el esquema v6, válida solo con `type_kind = "domain"`. |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Observaciones opcionales del catálogo en el esquema v6. La omisión significa desconocido; `false` confirma la ausencia de la propiedad. |
| `has_check` | bool | Observación opcional de un CHECK de una sola columna en el esquema v6. Cada `true` está cubierto por `check_count` de la tabla. |
| `null_fraction` | float | Fracción nula observada opcional entre `0.0` y `1.0`. Solo se conserva el agregado redondeado; no se conserva ningún mapa de bits de valores nulos. |
| `native_type` | string | Tipo base saneado opcional del motor, como `varchar` o `longtext`; sin identificadores, miembros de enumeraciones, valores predeterminados ni expresiones. Lo emiten los recopiladores nativos de MySQL y SQL Server. |
| `declared_max_chars` | int | Capacidad declarada opcional en caracteres. Exacta para los valores de catálogo `character`/`character varying` de PostgreSQL y en los modos MySQL equilibrado predeterminado y exacto; solo se redondea de forma aproximada con `--length-fidelity strict` de MySQL. |
| `declared_max_bytes` | int | Capacidad declarada opcional en bytes. Exacta en los modos MySQL equilibrado predeterminado y exacto; solo se redondea de forma aproximada con `--length-fidelity strict`. |
| `numeric_precision`, `numeric_scale`, `datetime_precision` | int | Precisión escalar opcional declarada por el motor. |
| `charset`, `collation` | string | Metadatos opcionales saneados de caracteres. MySQL emite los nombres de catálogo de su juego de caracteres y su intercalación. SQL Server emite `utf-16le` para `nchar`/`nvarchar`/`ntext`, `utf-8` para la página de códigos 65001, `windows-N` para las páginas de códigos de Windows 1250–1258 o `code-page-N` para cualquier otra página de códigos positiva del catálogo, además del nombre de intercalación del catálogo. Son hechos de codificación y nombres de catálogo, nunca identificadores ni valores del cliente. |
| `len_avg` | int | Promedio muestreado de bytes para valores de longitud variable. Los intervalos relativos predeterminados tienen un error máximo de aproximadamente el 3,2 % y conservan exactamente los valores de hasta 32 bytes; es exacto con `--length-fidelity exact --yes`; el redondeo aproximado a la decena más cercana solo se utiliza en modo estricto. 0 = longitud fija o sin medir. |
| `len_p95` | int | Percentil 95 muestreado con los mismos intervalos relativos predeterminados; exacto con `--length-fidelity exact --yes`; el redondeo aproximado a la centena más cercana solo se utiliza en modo estricto. 0 = sin medir. |
| `style` | string | Solo nivel 2. Uno de `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` o `"precompressed"`; vacío si no se clasifica. `"precompressed"` solo se emite para una muestra de valores binarios materialmente dominante en bytes que contenga firmas reconocidas de contenedores estándar. No revela deliberadamente la familia de contenedor detectada. |
| `magnitude_min`, `magnitude_max` | int | Exponentes decimales con signo opcionales del esquema v6 que delimitan la magnitud de los números no NULL muestreados. Se emiten con `has_negative`; nunca se serializan valores exactos. |
| `has_negative` | bool | Observación opcional del signo en el esquema v6, emitida solo con ambos límites de magnitud. |
| `time_span` | string | Intervalo opcional de fecha/hora muestreado en el esquema v6: `intraday`, `days`, `weeks`, `months`, `years` o `decades`. |
| `time_recent_decade` | int | Década de la fecha/hora muestreada más reciente en el esquema v6, emitida solo con `time_span` y siempre divisible por 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Solo nivel 2. Presente para columnas candidatas de texto o binarias que se hayan muestreado. Misma disposición de campos que la compresión por tabla, pero limitada a una columna anonimizada. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Resumen de la distribución de valores muestreados del esquema v3. Solo contiene recuentos y frecuencias acotados o redondeados. |

### `[tables.<id>.cols.<cid>.cardinality]` (esquema v3)

Cuando se habilita el muestreo de filas, el recopilador conserva en memoria
como máximo 8192 huellas temporales de 64 bits por columna, obtiene estadísticas
agregadas de NDV y sesgo, y descarta las huellas. No se serializan ni los valores
ni las huellas. El bloque contiene `measured`, `sample_rows`, `non_null_rows`,
`observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`,
`frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`,
`sample_method`, `sample_layout`, `sampled_with_bias` y `bias_reason`.
`sample_layout` es una enumeración opcional legible por máquina. El valor que
se emite actualmente es `primary-key-range-windows`; su ausencia significa que
no existe ningún contrato de orden. Los consumidores no deben inferir la
semántica de generación analizando el campo legible `sample_method`.

Los recuentos y las fracciones se redondean cuando corresponde para proteger la
privacidad. Las estadísticas sirven para reproducir la densidad de duplicados,
el sesgo de valores frecuentes y los dominios finitos en conjuntos de datos
sintéticos. No contienen valores muestreados, pero una distribución distintiva
puede identificar una carga de trabajo; no las considere irreversibles ni una
prueba de que no pueda inferirse el significado empresarial mediante
conocimiento externo.

### `[tables.<id>.cols.<cid>.compression]` (solo nivel 2)

La compresión por columna solo se emite para candidatos acotados de texto o
binarios cuando se utiliza `--measure-compression --yes`. Permite que las
herramientas posteriores generen datos sintéticos de texto o binarios con una
entropía más realista que la obtenida únicamente a partir de proporciones por
tabla.

El bloque contiene los mismos campos que `[tables.<id>.compression]`:
`measured`, `sample_rows`, `sample_bytes`, `sample_method`,
`sampled_with_bias`, `bias_reason`, `ratio_zstd_3`, `ratio_zstd_19`,
`ratio_stddev` y `sample_encoding`.

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
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

No se escribe en el archivo Blueprint ningún valor de columna muestreado.

Para las columnas binarias, la misma muestra acotada de nivel 2 puede emitir el
perfil general `style = "precompressed"`. El reconocimiento solo se realiza en
los límites de los valores muestreados y exige una observación materialmente
dominante en bytes. Blueprint no analiza ni descomprime el valor, no conserva
su firma y no distingue entre imágenes, archivos, medios comprimidos, datos
cifrados y cargas aleatorias más allá de esta única etiqueta de alta confianza.
Las codificaciones textuales y base64 siguen clasificándose por su estilo de
texto y no se consideran contenedores binarios precomprimidos.

El generador determinista asigna este perfil general a un contenedor ZIP neutro
y válido con un miembro almacenado de alta entropía. Así conserva el
comportamiento operativo de compresión sin afirmar que el origen contenía
archivos ZIP ni revelar si el valor de origen reconocido era una imagen, un
archivo o un contenedor multimedia. La generación binaria ordinaria no cambia.

## `[tables.<id>.idxs.<iid>]`

El identificador es `idx-N`, donde `N` es el ordinal indexado desde 1 del índice
dentro de la tabla, ordenado mediante un HMAC-SHA256 con separación por dominio
del nombre del índice.

| Campo | Tipo | Valores |
|---|---|---|
| `type` | string | Familia normalizada del método de índice, como `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` u `"other"`. No se emiten nombres de métodos personalizados o de extensiones. |
| `primary` | bool | Opcional; se emite como `true` para índices de clave primaria. Se omite o es `false` en los demás casos. |
| `unique` | bool | |
| `cols` | array of int | ordinales de las columnas participantes, en el orden de las columnas del índice |
| `prefix_lengths` | array of int | Longitudes opcionales de prefijos de índices MySQL alineadas con `cols`; cero significa la columna completa. Exactas de forma predeterminada; solo se redondean hacia abajo con `--length-fidelity strict`. |
| `include_cols` | array of int | Opcional; ordinales de columnas INCLUDE que no forman parte de la clave cuando el motor de origen los expone. |
| `expression` | bool | Opcional; true cuando existe material de clave de expresión o función que no puede representarse como simples ordinales de columna. |
| `filtered` | bool | Opcional; true para índices filtrados o parciales. |
| `descending` | bool | Opcional; true cuando alguna columna de clave está explícitamente en orden descendente. |
| `prefix_distinct_counts` | array of int | Recuento estimado por el esquema v3 de tuplas distintas para cada prefijo de clave, desde una hasta N columnas. Cero significa que no está disponible para ese prefijo. |
| `cardinality_sample_method` | string | Procedencia acotada de `prefix_distinct_counts`; los productos inferidos se etiquetan explícitamente y no se presentan como muestras directas de tuplas. |

## `[tables.<id>.compression]` y `[tables.<id>.cols.<cid>.compression]` (solo nivel 2)

Solo están presentes cuando el archivo se genera con
`--measure-compression --yes`. El bloque de nivel de tabla mide una proyección
columnar neutra de la muestra completa y sigue siendo la proporción autoritativa para las
estimaciones de transferencia de toda la tabla. Los bloques por columna se
proyectan a partir de las mismas filas muestreadas, columna a columna, y existen
para ayudar a los generadores posteriores de conjuntos de datos sintéticos a
ajustar la entropía por columna sin ver valores del cliente. No provocan lecturas
adicionales de la base de datos.

| Campo | Tipo | Precisión |
|---|---|---|
| `measured` | bool | siempre `true` si el bloque está presente |
| `sample_rows` | int | exacta |
| `sample_bytes` | int | tamaño del búfer de muestras en memoria, **agrupado por intervalos**: al múltiplo de **64 KiB** más cercano por debajo de 1 MiB, al **1 MiB** más cercano por debajo de 1 GiB y al **100 MiB** más cercano por encima. Los bytes nunca se escriben en disco. La agrupación elimina el canal encubierto de bits bajos por tabla que expondría un valor exacto de `buf.len()`. |
| `sample_method` | string | descripción del muestreo acotado específica del motor, por ejemplo `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`, `"LIMIT N (fallback after empty TABLESAMPLE)"` o `"SELECT TOP N"` |
| `sampled_with_bias` | bool | true si la muestra no es uniforme, por ejemplo un mecanismo alternativo que solo utilice LIMIT |
| `bias_reason` | string | vacío si `sampled_with_bias = false`; de lo contrario, una etiqueta como `"unordered_limit_after_empty_TABLESAMPLE"` |
| `ratio_zstd_3` | float | redondeada al múltiplo de **0.05** más cercano, según la política de medición zstd de nivel 3 del contrato. Medida sobre bytes codificados mediante `sample_encoding`. |
| `ratio_zstd_19` | float | proporción heredada de zstd nivel 19 aceptada de capturas antiguas; la herramienta ya no la mide ni la emite |
| `ratio_stddev` | float | redondeada al múltiplo de **0.05** más cercano, desviación estándar de las proporciones de nivel 3 sobre tramas acotadas de la sonda de tabla. Los bloques de proyección por columna emiten actualmente `0.0` porque son indicios orientativos de entropía, no un modelo de varianza. |
| `sample_encoding` | string | identificador de la codificación a nivel de bytes y de la política de sesión de compresión utilizadas para la medición. Los bloques de tabla activos de PostgreSQL usan actualmente `"blueprint-columnar-transfer-probe-v2"`. MySQL y SQL Server usan `"blueprint-columnar-transfer-probe-v3"`, que además vacía el contexto en los límites de bloques de sonda de 256 KiB. Las cargas `nvarchar`/`nchar`/`ntext` de SQL Server conservan la distribución nativa de bytes UTF-16LE; `varchar`/`char`/`text` conservan el ancho de bytes muestreado y el campo `charset` identifica la página de códigos del catálogo que necesita el consumidor. V1 sigue siendo un contrato de compatibilidad solo de entrada. Los bloques por columna usan `"blueprint-compression-probe-v2"`. El estimador de dbwarp DEBE validar esta cadena antes de consumir la proporción: distintas codificaciones o políticas de sesión NO son intercambiables. Los archivos Blueprint antiguos pueden no incluir este campo; los estimadores solo deben consumir proporciones medidas cuando la etiqueta de codificación esté presente y sea reconocida. |

El estimador de dbwarp debería preferir los bloques de compresión por columna
reconocidos al crear conjuntos de datos sintéticos, después recurrir a la
compresión por tabla y, por último, a los valores predeterminados de tipo y
estilo.

### Codificación a nivel de bytes `blueprint-compression-probe-v2`

El muestreador de nivel 2 concatena filas o valores de columnas muestreados en
un búfer en memoria con este formato y, a continuación, ejecuta zstd de nivel 3
sobre él. El búfer se descarta. El Blueprint conserva únicamente los campos
agregados documentados de compresión, densidad de valores nulos,
cardinalidad/frecuencia, longitud y estilo.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Las etiquetas de tipo forman parte del contrato de la sonda y no se
renumerarán sin un nuevo identificador de sonda versionado.

| Etiqueta | Nombre | Se utiliza para |
|---|---|---|
| 0x00 | Null | SQL NULL (sin longitud ni carga útil) |
| 0x01 | TextUtf8 | texto UTF-8 |
| 0x02 | TextUtf16Le | bytes UTF-16LE, principalmente SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | bytes en otro juego de caracteres |
| 0x04 | NumberText | representación textual decimal de valores numéricos |
| 0x05 | BoolText | booleano como texto |
| 0x06 | TimestampText | texto de marca de tiempo ISO-8601 |
| 0x07 | DateText | texto de fecha ISO-8601 |
| 0x08 | TimeText | texto `HH:MM:SS[.fff]` |
| 0x09 | UuidText | texto canónico de UUID de 36 caracteres |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | bytes de `bytea`, `varbinary`, `image` o blob |
| 0xFE | UnknownText | representación textual alternativa proporcionada por la base de datos |

### Codificación a nivel de bytes `blueprint-columnar-transfer-probe-v1`, `v2` y `v3`

Las proporciones de tabla de bases de datos activas transforman las mismas
muestras acotadas por columna v2 en tramas neutras de 1000 filas. Cada trama
tiene un encabezado de sonda versionado y, por cada columna, un ordinal, una
etiqueta de tipo, una longitud de cuatro bytes por fila y, después, los bytes de
carga útil contiguos por columna. Una longitud de `0xffffffff` representa NULL.
La representación de bytes es común a las tres versiones. V1 comprimía la
secuencia de tramas unida como una única operación zstd de nivel 3 con el tamaño
de entrada declarado. V2 pasa las tramas por un contexto zstd de nivel 3
persistente y lo vacía después de cada trama. V3 conserva ese contexto y la
representación neutra por grupos de filas, pero también lo vacía en cada límite
de fragmento de compresión de sonda de 256 KiB dentro de un grupo. La captura
de MySQL y SQL Server usa v3; v2 sigue siendo la medición actual de PostgreSQL.
El texto Unicode de SQL Server se mide como UTF-16LE. El texto estrecho de SQL Server conserva el
ancho de bytes de origen y registra un juego de caracteres cerrado y saneado
derivado de la página de códigos de la intercalación. Las salidas de los grupos
de filas exteriores proporcionan observaciones de `ratio_stddev`. Las etiquetas
versionadas evitan reinterpretar silenciosamente una política de tramas o de
vaciado como otra.

Esta representación modela propiedades genéricas relevantes para la compresión
de una transferencia masiva columnar. No es una captura del protocolo de base
de datos, un formato de transporte de migración ni una exportación de datos
codificada. Los bytes muestreados permanecen solo en memoria y se descartan una
vez derivadas las mediciones agregadas.

### Límites de precisión

`ratio_zstd_3` describe el `sample_encoding` indicado; no mide los bytes del protocolo de base de datos ni del transporte de migración. El conjunto público de pruebas automatizadas valida la codificación determinista, el muestreo acotado y la serialización, pero no afirma un porcentaje de error universal para todos los motores y métodos de extracción.

Antes de utilizar la proporción para una decisión de capacidad importante, valide el binario y la versión actuales del motor con datos de origen representativos y el mecanismo de extracción previsto. Registre con el plan resultante el método de comparación, el tamaño de la muestra, el hash del binario, la versión del motor y el error observado. La relación primitiva es `compressed_bytes ≈ sample_bytes / ratio_zstd_3` bajo la distribución de bytes producida por el `sample_encoding` registrado.

## `[fk_edges]`

Opcional. Tabla en línea en la que cada clave es un identificador `table-NNN`
asignado a una lista de aristas. El esquema v3 conserva los ordinales del padre,
las acciones referenciales, el modo de coincidencia, la posibilidad de diferir,
el estado de validación/confianza y un resumen opcional, acotado y sin nombres,
de la relación. Las aristas se ordenan primero por destino y después por
lista de columnas.

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

El bloque opcional `statistics` registra valores muestreados o inferidos de
`non_null_rows`, `distinct_parent_values`, `parent_coverage_fraction`, fanout
p50/p95/p99/max y `orphan_rows`, además de campos de procedencia y sesgo. Las
restricciones de origen validadas implican cero huérfanos. Las estimaciones
compuestas derivadas de muestras por columna se marcan explícitamente como
inferidas. Los generadores usan estos agregados para reproducir la cobertura
NULL y el fanout, asignando cada clave secundaria compuesta a una tupla primaria
sintética coherente.

## `[artifact_inventory]` (desde el esquema v4, orígenes de bases de datos)

El contrato versionado de forma independiente `dbwarp-blueprint-artifacts/v1`
describe objetos no tabulares sin serializar nombres de origen ni definiciones.
No aparece para archivos estructurados ni al seleccionar `--artifact-detail none`.

El valor predeterminado `--artifact-detail summary` emite `object_count`,
`external_prerequisite_count`, `counts_by_kind` y
`counts_by_external_class`. `graph` añade un registro de objeto anónimo por
artefacto y aristas de dependencia. `analyzed` añade registros acotados de
`dbwarp-language-feature-census/v1` derivados temporalmente de las definiciones
disponibles. `graph` y `analyzed` requieren `--yes` explícito porque la
topología del grafo puede identificar una aplicación.

La evidencia del inventario incluye:

| Campo | Valores / regla |
|---|---|
| `detail` | `none`, `summary`, `graph` o `analyzed` |
| `visibility` | `full`, `privilege_filtered` o `unknown` |
| `inventory_complete` | Solo puede ser verdadero con visibilidad completa, sin catálogos ilegibles ni familias sin modelar declaradas |
| `dependencies_complete` | Solo puede ser verdadero si se pudieron leer los catálogos de dependencias modelados |
| `analysis_complete` | Solo puede ser verdadero con detalle analyzed y si todos los análisis emitidos están completos |
| `catalogs_read` | Etiquetas cerradas y estándar de catálogos de motor inspeccionados correctamente |
| `catalogs_unreadable` | Etiquetas de catálogos fallidos; cualquier entrada impide afirmar integridad completa |
| `families_not_inventoried` | Familias conocidas fuera del contrato actual del colector |

Los identificadores de objeto tienen la forma `<kind>-NNN`, como `view-001` o
`function-002`. El registro solo contiene tokens cerrados de kind, subkind y
tier, identificadores anónimos de esquema/padre, dependencias anónimas, número
de dependencias no resueltas, visibilidad de definición y modo de seguridad
acotados, un requisito externo opcional y un censo opcional de características del lenguaje de programación. Los
nombres de objetos de origen, texto SQL, entidades de seguridad, puntos de
conexión, credenciales, claves, certificados y binarios no son campos del contrato.

Los requisitos externos registran una `class` cerrada, el ámbito de despliegue,
si se requiere material binario/secreto/de punto de conexión no capturado y una
categoría de compatibilidad acotada. Su recuento es evidencia de planificación
de la migración, no una afirmación de que DBWarp pueda aprovisionarlos o
traducirlos automáticamente.

Los registros del censo de características del lenguaje de programación usan `analyzer_version = "lexical-v1"` y
`status = "partial"`. Los valores de recuento, tamaño, anidamiento, complejidad
y regiones opacas son bandas, no huellas exactas del origen. Las características
proceden de un vocabulario cerrado. El analizador elimina comentarios,
literales e identificadores entre comillas; no es un analizador sintáctico, un
enlazador semántico ni una garantía de traducción correcta.

Consulte el [Inventario de artefactos no tabulares](ARTIFACT_INVENTORY.md) para
la guía operativa y la cobertura de motores.

## Defensas contra la esteganografía, por vector

| Vector | Defensa |
|---|---|
| Orden de los identificadores | HMAC-SHA256 con separación por dominio y una clave secreta local al proceso impide comprobar nombres candidatos sin conexión. Reutilice una clave custodiada por el cliente solo cuando se necesiten etiquetas estables entre ejecuciones. |
| Bits bajos numéricos | Las estadísticas se redondean de forma predeterminada con la precisión documentada. El modo de longitudes exactas es explícito, requiere consentimiento, se registra en el registro de auditoría y debe tratarse como metadatos más sensibles. |
| Marca de tiempo inferior a un segundo | Una marca de tiempo UTC en la cabecera, solo con resolución de segundos |
| Formato TOML | Canónico: claves alfabéticas, sangría fija y solo comentarios fijos de cabecera/productor; sin comentarios derivados de la entrada |
| Aleatoriedad del muestreo | El muestreo utiliza semillas fijas (`TABLESAMPLE SYSTEM` determinista de PG). Por separado, la anonimización de identificadores obtiene deliberadamente una clave secreta del CSPRNG del sistema operativo, salvo que el cliente proporcione una. |
| Campos sin utilizar | Todos los campos se documentan arriba; no hay campos "metadata"/"comment"/"reserved" que contengan datos sin límites |
| Texto fuente de artefactos y material externo | Las definiciones son transitorias y se borran tras el análisis acotado; nombres, texto SQL, puntos de conexión, cadenas de proveedor, credenciales, claves, certificados, nombres de paquetes y binarios no tienen ningún campo serializado |

## Compatibilidad de versiones del esquema

Los productores actuales emiten la versión 6 del esquema. Las versiones 1 a 5
siguen aceptándose por compatibilidad. Un archivo v1/v2 no contiene bloques de
distribución, por lo que los generadores usan alternativas deterministas de
tipo, anchura y relaciones uniformes e informan de la pérdida de fidelidad. Un
archivo v3 tiene metadatos de distribución, pero no un inventario de artefactos.
Un archivo v4 puede contener un inventario de artefactos, pero es anterior a los
identificadores actuales del contrato Blueprint. Los lectores normalizan los
identificadores v4 anteriores al leer y vuelven a emitir el documento con
identificadores Blueprint canónicos. Un archivo v5 es anterior a la calificación
de la topología y el alcance del conjunto de datos añadida en v6. Los consumidores deben rechazar
versiones futuras desconocidas con un mensaje claro de actualización, en lugar
de descartar campos silenciosamente.

## Por qué TOML y no JSON

- TOML separa de forma más legible las secciones estructurales de los datos de
  hoja (`[tables.table-001.cols.col-2]` frente a JSON anidado).
- Es más fácil de comparar (una clave por línea; las subtablas basadas en
  identificadores permanecen contiguas).
- El cliente puede editarlo manualmente si desea ocultar un campo específico
  antes de compartirlo.

JSON se utiliza como **formato intermedio** en la ruta SQL alternativa. Cada
script `sql/blueprint.*.sql` produce JSON y `blueprint_format.py` lo normaliza a
TOML. El JSON intermedio contiene identificadores reales del origen; MySQL
también puede incluir declaraciones enum/set mediante `COLUMN_TYPE`, por lo que
debe permanecer protegido dentro del entorno de origen. El normalizador usa una
clave secreta nueva de forma predeterminada y acepta el mismo contrato protegido
de `--anonymization-key-file` para comparaciones aprobadas entre ejecuciones. El
archivo final revisado para compartirlo con DBWarp siempre es TOML.

## Extensiones de procedencia para archivos estructurados

Cuando `engine` o `source_kind` es `"parquet"` o `"avro"`, la versión 3 del
esquema y las posteriores también pueden emitir los siguientes campos acotados.
Los lectores antiguos deben ignorar los campos que no comprendan; los lectores
más recientes deben preservar la distinción entre el almacenamiento del archivo
de origen y las mediciones acotadas de muestras decodificadas.

Los Blueprints de archivos estructurados usan los mismos identificadores
anonimizados que las Blueprints de bases de datos: `table-NNN` en orden basado en una clave secreta
y `col-N` en orden ordinal del esquema. Los nombres base de archivo,
las rutas Parquet, los nombres de campo Avro y el valor `logical_table` del
manifiesto no se emiten como identificadores de tabla o columna.

En una tabla, `table_bytes` es la estimación lógica para dimensionar la
transferencia y `storage_bytes` es el tamaño real del objeto de origen. Parquet
sin muestreo decodificado usa los bytes sin comprimir de los fragmentos de
columna para `table_bytes`; el muestreo
decodificado opcional los sustituye por bytes `blueprint-compression-probe-v2`
proyectados. Avro deriva el valor de su recorrido decodificado completo.
`source_partitions`, `row_group_count` y `source_codec` describen la organización
y la procedencia de planificación. Los conjuntos de varios archivos agregan
estos valores. `row_group_count` es específico de Parquet y `source_partitions`
es `1` para un único objeto de entrada.

En una columna, `null_fraction` es un valor observado entre `0.0` y `1.0`.
`length_sample_rows` y `length_sample_method` indican cómo se obtuvieron
`len_avg` y `len_p95`. `source_semantics` conserva hechos acotados de
compatibilidad como `"repeated-leaf"`, `"nested-json"` o `"multi-type-union"`;
nunca contiene el nombre ni el valor de un campo del cliente. La precisión y la
escala decimales, la precisión y semántica UTC/local de marcas de tiempo, UUID
y el tamaño binario fijo se conservan en los campos escalares saneados
existentes y `native_type`.

En una tabla, `ratio_storage` compara `table_bytes` con los bytes reales del
objeto de origen. En una columna Parquet compara los bytes sin comprimir y
comprimidos del fragmento de columna del footer. Ambos son señales para
planificar el almacenamiento de archivos, no estimaciones de muestras
decodificadas. `ratio_zstd_3` y `ratio_zstd_19` solo son entradas válidas de calibración
de transferencias cuando `sample_encoding` es
`"blueprint-compression-probe-v2"`. Los ratios del footer Parquet o del contenedor
Avro nunca deben copiarse a esos campos zstd.
