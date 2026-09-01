# Qué lee y escribe dbwarp-blueprint

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../../AUDIT.md).

**Idiomas:** [English](../../AUDIT.md) | [Deutsch](../de/AUDIT.md) | [Français](../fr/AUDIT.md) | **Español** | [Polski](../pl/AUDIT.md) | [日本語](../ja/AUDIT.md) | [中文](../zh/AUDIT.md)

Este documento enumera todas las acciones que puede realizar la herramienta. Contrástelas
con su política de seguridad.

## Salida de red

El modo en vivo `--connect` abre una sesión del controlador de base de datos con el punto de conexión indicado. La resolución DNS puede utilizar el solucionador configurado, y la autenticación Kerberos/SSPI integrada puede contactar con un KDC o un controlador de dominio. El modo por lotes procesa sus orígenes secuencialmente y abre una sesión por cada origen de base de datos. Las operaciones sin conexión con TOML, Parquet, Avro y paquetes no abren ninguna conexión de red iniciada por la aplicación, aunque una ruta en un sistema de archivos de red sigue dependiendo de la pila de almacenamiento del host.

El binario no tiene telemetría, comprobación de licencias, actualización de versiones, llamadas a API de nube ni rutas de carga.

Puede verificarlo mediante `strace -f -e trace=connect,sendto,recvfrom`,
`tcpdump` o eBPF en la plataforma que prefiera.

## Lecturas del sistema de archivos

La herramienta lee las entradas seleccionadas por el modo activo:

| Archivo | Cuándo | Contenido |
|---|---|---|
| `--user-file PATH` | Si se proporciona | Solo el nombre de usuario. Se elimina el espacio en blanco final; un archivo vacío es un error. |
| `--password-file PATH` | Si se proporciona | Se lee una vez y se pone a cero después de usarlo. Se rechaza si el modo permite la lectura a todo el mundo o al grupo. |
| `--azure-token-file PATH` | Si se proporciona | Token de SQL Server Entra ID. Se lee una vez y se pone a cero después de usarlo. Se rechaza si el modo permite la lectura a todo el mundo o al grupo. |
| `--tls-ca PATH` | Si se proporciona | CA de confianza en formato PEM que se lee al establecer la conexión. PostgreSQL/MySQL aceptan un paquete; SQL Server acepta exactamente un certificado. El archivo proporcionado sustituye las raíces predeterminadas del motor. |
| `--tls-cert PATH` | Si se proporciona | Certificado TLS de cliente para PostgreSQL/MySQL (PEM), leído al establecer la conexión. Se rechaza para SQL Server con `DBP1015E`. |
| `--tls-key PATH` | Si se proporciona | Clave TLS de cliente para PostgreSQL/MySQL (PEM). Se rechaza si el modo permite la lectura a todo el mundo o al grupo. Se lee al establecer la conexión y se rechaza para SQL Server con `DBP1015E`. |
| `--from-toml PATH` | Si se proporciona | Archivo TOML existente de dbwarp-blueprint, leído localmente para crear una presentación sin conexión a una base de datos. |
| `--from-parquet PATH` | Si se proporciona | Metadatos de Parquet y, solo con consentimiento explícito para el muestreo, filas decodificadas acotadas. |
| `--from-avro PATH` | Si se proporciona | Metadatos y registros del contenedor Avro; se recorre el contenedor para obtener el recuento de filas. |
| `--batch-manifest PATH` | Si se proporciona | Manifiesto y todas las rutas locales de entrada, credenciales, tokens y TLS que referencia. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Si se proporciona | TOML del paquete y archivos Blueprint relativos necesarios para enumerar, extraer o empaquetar. |
| `/dev/tty` | Si no se proporciona ninguna fuente de contraseña | Solicitud con eco deshabilitado. |
| (solo durante la compilación) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` en versiones con dependencias incluidas, `vendor/mysql_async`, `vendor-crates/*` en paquetes sin conexión | Solo cuando se ejecuta `./build.sh` | Entradas de toolchain, procedencia del código y compilación Cargo |

Qué **NO** lee:
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- Ningún archivo `~/.ssh/*`
- `/etc/passwd`, `/etc/shadow`
- Ninguna variable de credenciales de base de datos salvo la indicada mediante `--password-env`,
  `--user-env` o `--azure-token-env`. Las compilaciones con Kerberos integrado
  también pueden observar `KRB5CCNAME` porque libgssapi utiliza la caché de
  tickets de Kerberos. Las variables de idioma y presentación del terminal se describen abajo.

## Escrituras en el sistema de archivos

La herramienta escribe únicamente las salidas seleccionadas por el modo activo:

| Archivo | Cuándo | Contenido |
|---|---|---|
| `--out PATH` (valor predeterminado `./blueprint.toml`) | Ejecuciones de base de datos en vivo, Parquet, Avro, extracción de paquetes y empaquetado de paquetes | TOML Blueprint o de paquete empaquetado. No se escribe en modos de solo presentación, enumeración de paquetes, simulación, ayuda o versión. |
| `--deck PATH` | Solo si se especifica | Una presentación de PowerPoint (.pptx) que resume el Blueprint anonimizado. Se crea localmente a partir del mismo Blueprint en memoria o de la entrada `--from-toml`: sin lectura adicional de la base de datos, sin red y sin biblioteca de terceros. |
| `--audit-log PATH` | Solo si se especifica | Una copia sustituida atómicamente del registro de auditoría emitido en stderr; no se añade al contenido anterior. |
| `--out-dir DIR` | Modo por lotes que no sea una simulación | `bundle.toml`, directorios `blueprints/` y `audits/` por origen, un marcador de propiedad y `errors.txt` tras un error parcial. La publicación utiliza un directorio de preparación adyacente y un marcador de recuperación. |
| (solo durante la compilación) `./target/`, `./build/` | Solo cuando se ejecuta `./build.sh` | Salidas de compilación estándar de Cargo |

Qué **NO** escribe:
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- ningún directorio temporal del sistema implícito (el usuario puede dirigir allí explícitamente una salida o un directorio por lotes)

## Variables de entorno leídas

La auditoría enumera solo las variables realmente consultadas. Si `--lang` no
selecciona un idioma compatible, la selección puede leer `DBWARP_BLUEPRINT_LANG`, `LC_ALL`,
`LC_MESSAGES` y `LANG`. La presentación del terminal puede leer `NO_COLOR`,
`TERM`, `COLORTERM` y `COLUMNS`; solo afectan a la presentación.

Cuando se especifica `--password-env VAR_NAME` o `--user-env VAR_NAME`,
la herramienta lee exactamente esa variable. No recurre a valores
predeterminados habituales como `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`,
`USER` o `LOGNAME`; esas alternativas no se han implementado
deliberadamente.

Cuando se ejecuta `./build.sh`, se leen `PINNED_RUST` (sustitución), `ALLOW_NETWORK`
(opción explícita para descargar rustup-init), `TARGET` (destino de compilación cruzada), además
de las variables estándar de cargo/rustup. La propia herramienta no lee ninguna
de ellas durante la ejecución.

## Registro de auditoría de cada ejecución

La herramienta emite un registro de auditoría en stderr en cada ejecución. El formato es
texto sin formato determinista. Rediríjalo a un archivo con `2>audit.txt` o utilice
`--audit-log PATH` para obtener una copia explícita.

Ejemplo (nivel 1):

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (the DB connection only)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - all numeric statistics rounded to documented precision
  - identifier ordering is deterministic (sha256-based)
  - no random or pseudorandom data in output
  - artifact summary stores bounded counts only; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential read once via Secret wrapper, zeroized when dropped at end of engine run; see SECURITY.md for driver-owned copy lifetimes (MySQL clones to non-zeroizing String for the driver API)

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

Las ejecuciones de MySQL emiten una afirmación específica del modo
`length policy balanced|strict|exact`. Indica de manera independiente si las
longitudes estructurales y muestreadas son exactas o redondeadas, de modo que la
auditoría nunca afirma que todos los valores numéricos se redondearon en una
ejecución balanced o exact.

El registro de auditoría:

- Registra solo el número de selectores repetibles de captura en vivo `--schema`; sus valores aparecen en la comprobación previa interactiva, pero no se añaden a la auditoría. El URI de conexión redactado existente sigue identificando la base de datos conectada, que también es el nombre del esquema en MySQL. Un Blueprint seleccionado se marca como `selection-limited` en `dataset_scope`.
- identifica la revisión del código fuente integrada al compilar y el estado del árbol de trabajo; el SHA-256 final del binario sigue siendo una suma externa de la versión o del registro, porque un binario no puede incorporar su propio hash final;
- Registra la **fuente** de la credencial (ruta de archivo, nombre de variable de entorno,
  TTY), nunca el valor.
- En SQL Server registra las identidades exactas de sesión devueltas por
  `ORIGINAL_LOGIN()`, `SUSER_SNAME()` y `USER_NAME()`. Cuando se proporciona
  `--expect-server-principal`, también registra el valor esperado y si la
  comparación del servidor coincidió antes de capturar el catálogo.
- Enumera cada operación de base de datos observada con su resultado, duración y recuento de filas cuando el controlador lo proporciona; los fallos terminales usan una etiqueta acotada sin identificadores.
- Informa los bytes de red como `unknown` cuando el controlador no los expone y separa los bytes de muestra codificados localmente.
- Informa de los bytes totales escritos localmente (con sha256 de cada archivo).
- Registra degradaciones no fatales de captura y muestreo mediante códigos de
  advertencia DBP estables; una sección vacía significa que no se observó ninguna
  degradación conocida.
- Copia la evidencia validada de `[database_topology]` y `[dataset_scope]` en `topology_and_scope` usando solo tokens cerrados y recuentos; no pueden aparecer nombres de nodo, endpoints ni identificadores de clúster o base.
- Conserva `DBP1411W`, `DBP1412W` y `DBP1413W` cuando la topología o la cobertura es incompleta, para que una captura correcta no oculte una salvedad de dimensionamiento.
- Registra una estimación determinista y desglosada por dimensiones de la fidelidad de Blueprint. La puntuación describe la cobertura de la evidencia capturada para estructura, dimensionamiento, estadísticas de columnas, relaciones y artefactos. No es un error medido frente a los datos fuente ni un intervalo de confianza estadístico.
- Declara afirmaciones de confianza adecuadas al modo (nivel 1 frente a nivel 2).
- Es determinista para la misma entrada: misma base de datos y mismos argumentos producen la misma auditoría,
  salvo los campos temporales.

**Emisión condicional de afirmaciones de confianza.** La línea
"credential read once via Secret wrapper..." solo se emite en ejecuciones
en las que realmente se leyó una credencial. Las rutas de error que terminan
antes de adquirir credenciales (errores al analizar la URI, rechazo de
contraseñas incrustadas en la URI, simulación, etc.) deliberadamente *no* emiten
esta línea: no hay nada que afirmar sobre una credencial que nunca se obtuvo.
Utilice la presencia o ausencia de la línea junto con
`auth.password_source` para saber si se ejercitó el tratamiento de credenciales
en una ejecución determinada.

**La auditoría se emite en las rutas operativas de éxito y error**, incluidos
los errores de análisis de la línea de comandos posteriores al inicio. Las
salidas de ayuda/versión y los fallos anteriores a la carga del contrato de
localización integrado no producen una auditoría completa. Los fallos
posteriores se siguen escribiendo en stderr y en `--audit-log PATH` si se indicó, con la forma `outcome: error: <stage>`.
Ejemplo de línea de resultado de error:

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

La salida de terminal también incluye un resumen codificado para el operador, como
`DBP1001E` o `DBP0001E`, junto con la cadena causal. El resultado de auditoría
está acotado y puede truncar texto largo; utilice la salida de terminal y el
código de mensaje para clasificar la incidencia de soporte. Consulte `docs/MESSAGES.md`.

Las sondas opcionales de RTT, compresión y estilo de texto pueden fallar sin invalidar
la captura principal del catálogo. Esos casos se imprimen y se conservan en
`warnings:` como `DBP1405W` a `DBP1408W`, de forma que un resultado de nivel 2
correcto pero parcial pueda distinguirse de uno completo. Las advertencias
idénticas repetidas se deduplican y los detalles multilínea del controlador se
aplanan para mantener la auditoría acotada y apta para procesamiento automatizado.

## Lecturas de artefactos no tabulares

La captura de artefactos es independiente del muestreo de filas de nivel 2:

- `--artifact-detail none` omite catálogos de artefactos y definiciones.
- `summary` lee catálogos de objetos modelados, pero no el texto de las definiciones.
- `graph` también lee catálogos de dependencias, pero no el texto de las definiciones.
- `analyzed` también lee definiciones SQL/procedimentales disponibles en memoria de proceso acotada para el análisis léxico.

La auditoría registra el detalle solicitado, la visibilidad, los recuentos de objetos, dependencias y requisitos externos, y todos los indicadores de integridad. Cada operación de catálogo aparece en `database_operations_observed`. Un catálogo opcional fallido emite `DBP1410W`, aparece en `warnings` e impide una afirmación de integridad inexacta.

En modo analizado, las definiciones se guardan en un propietario que las borra y se reducen a bandas acotadas y tokens de características cerrados. El texto de las definiciones, los nombres de objetos de origen, los puntos de conexión externos, las entidades de seguridad de artefactos, las credenciales, el material de claves/certificados, los nombres de paquetes/bibliotecas y los binarios nunca se escriben en el Blueprint ni en el registro de auditoría. Los únicos nombres exactos de entidades de seguridad que se conservan son las tres identidades de sesión de SQL Server del bloque de auditoría `auth` explícito anterior; nunca se escriben en el Blueprint, la presentación ni los artefactos publicados. Los modos graph y analyzed requieren `--yes`, porque la topología anónima puede identificar una aplicación.

La auditoría distingue las posturas de privacidad con una de estas afirmaciones de confianza:

- summary: solo recuentos acotados, sin identidades de objetos ni definiciones;
- graph: grafo anónimo de dependencias, sin definiciones;
- analyzed: definiciones leídas temporalmente, solo se conservan bandas acotadas.

Consulte [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) para la cobertura de familias de objetos y la interpretación de la integridad.

## Adiciones del nivel 2

Cuando la medición se acepta interactivamente, o sin interacción con `--measure-compression --yes`, la herramienta además:

- Para cada tabla que no se haya demostrado vacía, ejecuta una ruta de muestreo
  acotada específica del motor. PostgreSQL comienza con
  `TABLESAMPLE SYSTEM(0.1) LIMIT N` y recurre a `LIMIT N` cuando es necesario;
  MySQL usa `LIMIT N` y SQL Server `TOP N`. Las rutas sesgadas marcan
  `sampled_with_bias = true` en la salida.
- Lee las filas muestreadas en un búfer local en memoria.
- Mantiene secuenciales las lecturas de la base de datos. La opción
  `--compression-workers N` puede ejecutar de 1 a 32 workers locales acotados
  (1 de forma predeterminada para minimizar el impacto en el host de origen).
  Auméntelo explícitamente para utilizar más CPU local. Cada worker posee sus
  contextos zstd, sin un bloqueo zstd compartido.
- Comprime con zstd en el nivel 3.
- Registra las proporciones resultantes y la desviación estándar.
- **Descarta cada búfer cuando termina su trabajo local acotado**. Los bytes no
  se escriben en disco ni se transmiten. El grupo conserva como máximo N
  muestras en cola y N muestras en compresión activa.

`local_sample_processing.encoded_rowframe_bytes` muestra los bytes codificados
localmente para la compresión, no los bytes de red de la base. Los bytes que el
controlador no expone siguen como `unknown`. El bloque `[compression]` contiene las proporciones. `--max-wall-secs` es un plazo
estricto para toda la captura en vivo, incluida conexión, catálogos, RTT y Tier 2.
PostgreSQL también establece `statement_timeout` para la sesión; MySQL establece
`max_execution_time` para las sentencias `SELECT` de solo lectura; SQL Server
establece `LOCK_TIMEOUT` porque no tiene un límite de sesión equivalente para el
tiempo transcurrido de una sentencia. Al vencer el plazo exterior, el cliente
cierra la conexión. La auditoría no considera ese cierre una prueba de que SQL
Server haya confirmado la cancelación, por lo que un operador debe confirmar
que el trabajo del servidor se detuvo antes de reintentarlo.

`sampling_work` es evidencia operativa sin identificadores. Registra los
límites de workers y de cola locales, el límite de carga proyectada de 16 MiB
por tabla, los trabajos enviados y terminados, los
intentos de compresión y las tablas cuyo muestreo se omitió porque el catálogo
del motor demostró que estaban vacías al leerlo. `compression_worker_ms` es
tiempo de pared agregado de los workers, no tiempo de CPU del proceso, y puede
superar `compression_pipeline_wall_ms` cuando los workers se solapan. El
tiempo de pared del pipeline puede solaparse con las lecturas de base de datos,
que siguen siendo secuenciales. Estos contadores describen el trabajo
realizado; no son recuentos de filas, medidas de bytes de red ni afirmaciones
sobre la exactitud del origen.

## Protocolo de verificación

Si desea *demostrar* que la herramienta solo hace lo documentado:

1. **Auditoría del código fuente**: clone el repositorio, lea `src/secret.rs` y, a continuación, busque
   `\.expose\(\)` fuera de ese archivo:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   Las ubicaciones de llamada en producción entregan inmediatamente el `&str`
   expuesto al constructor de conexiones. MySQL también llama a `.to_string()`
   porque la API de `mysql_async` requiere
   `String`; esa copia no se pone a cero y vive hasta que se elimina
   `OptsBuilder`. Tier 1 y Tier 2 reutilizan la misma conexión MySQL. Consulte SECURITY.md §2.
2. **Compilación desde el código fuente**: `./build.sh`. La CI de publicación realiza una reconstrucción independiente en el mismo runner y en otro directorio de destino de Cargo, y rechaza cualquier diferencia de bytes. Una comparación local solo es significativa con la misma revisión del código fuente, destino, funciones, cadena de herramientas Rust fijada, enlazador y opciones de compilación.
3. **Comparación con la versión**: `./verify.sh release/dbwarp-blueprint-X.Y.Z-...`
4. **Seguimiento durante la ejecución**: ejecute con `strace -f -e trace=open,connect,read,write`
   en un entorno aislado. Compare el resultado con las listas anteriores.
5. **Seguimiento de red**: utilice `tcpdump` en el host. En una ejecución en vivo autenticada mediante contraseña, verifique la sesión de base de datos y el tráfico DNS esperado. Para la autenticación integrada, tenga en cuenta también el tráfico esperado hacia el KDC o el controlador de dominio. En el modo por lotes, reconcilie una sesión de base de datos por cada origen de base de datos.

Si alguno de estos resultados no coincide con lo documentado aquí, abra una incidencia con
el seguimiento correspondiente y la investigaremos en un plazo de 72 horas.
