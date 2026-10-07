# Qué lee y escribe dbwarp-blueprint

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../../AUDIT.md).

**Idiomas:** [English](../../AUDIT.md) | [Deutsch](../de/AUDIT.md) | [Français](../fr/AUDIT.md) | **Español** | [Polski](../pl/AUDIT.md) | [日本語](../ja/AUDIT.md) | [中文](../zh/AUDIT.md)

Este documento describe el comportamiento de red, sistema de archivos, entorno,
base de datos y auditoría de la aplicación durante la ejecución. Contraste cada
modo y opción activos con su política de seguridad. Los controladores de base de
datos, las bibliotecas TLS y de identidad, el cargador dinámico y el sistema
operativo también pueden consultar la configuración de la plataforma, los
almacenes de confianza, DNS, las cachés de credenciales y el almacenamiento en
red; esas acciones de la capa de soporte no son completamente visibles en la
auditoría de la aplicación.

## Salida de red

El modo en vivo `--connect` para PostgreSQL, MySQL y SQL Server abre una sesión del controlador de base de datos con el punto de conexión indicado. El modo por lotes procesa sus orígenes secuencialmente y abre una sesión por cada origen de base de datos. La resolución DNS puede utilizar el solucionador configurado, y la autenticación Kerberos/SSPI integrada puede contactar con un KDC o un controlador de dominio. Las operaciones sin conexión con TOML, Parquet, Avro y paquetes no abren ninguna conexión de red iniciada por la aplicación, aunque una ruta en un sistema de archivos de red sigue dependiendo de la pila de almacenamiento del host.

La vista previa de Oracle sujeta a confirmación inicia únicamente el ejecutable SQL*Plus indicado explícitamente mediante `--oracle-sqlplus` (incluida una prueba limitada `-V` cuando esté disponible) y lo utiliza para la sesión del catálogo. El proceso hijo recibe un directorio privado vacío como `TNS_ADMIN`; sus credenciales se escriben en la entrada estándar y nunca se colocan en los argumentos del proceso. El entorno se borra antes del inicio. Solo se reenvían `PATH`, `SystemRoot`, `WINDIR`, `ORACLE_HOME`, `LD_LIBRARY_PATH`, `DYLD_LIBRARY_PATH`, `LIBPATH` y `SHLIB_PATH` cuando están presentes; el recopilador establece por separado su configuración regional fija, la zona horaria y los valores privados de `TNS_ADMIN`.

El binario no tiene telemetría, comprobación de licencias, actualización de versiones, llamadas a API de nube ni rutas de carga.

Puede verificarlo mediante `strace -f -e trace=connect,sendto,recvfrom`,
`tcpdump` o eBPF en la plataforma que prefiera.

## Lecturas del sistema de archivos

La herramienta lee las entradas seleccionadas por el modo activo:

| Archivo | Cuándo | Contenido |
|---|---|---|
| `--user-file PATH` | Si se proporciona | Solo el nombre de usuario. Se elimina el espacio en blanco final; un archivo vacío es un error. |
| `--password-file PATH` | Si se proporciona | Se lee una vez. El búfer `Secret` propiedad de DBWarp se pone a cero al destruirse; los controladores pueden conservar sus propias copias como se documenta en SECURITY.md. Se rechaza si el grupo u otros pueden leerlo en Unix. |
| `--anonymization-key-file PATH` | Si se proporciona | Una clave HMAC de 32 bytes o 64 caracteres hexadecimales que usted custodia. Se rechaza si el grupo u otros pueden leerla en Unix. La clave nunca se emite. |
| `--azure-token-file PATH` | Si se proporciona | Token de SQL Server Entra ID. Se lee una vez; el búfer `Secret` propiedad de DBWarp se pone a cero al destruirse. Se rechaza si el grupo u otros pueden leerlo en Unix. |
| `--tls-ca PATH` | Si se proporciona | CA de confianza en formato PEM que se lee al establecer la conexión. PostgreSQL/MySQL aceptan un paquete; SQL Server acepta exactamente un certificado. El archivo proporcionado sustituye las raíces predeterminadas del motor. |
| `--tls-cert PATH` | Si se proporciona | Certificado TLS de cliente para PostgreSQL/MySQL (PEM), leído al establecer la conexión. Se rechaza para SQL Server con `DBP1015E`. |
| `--tls-key PATH` | Si se proporciona | Clave TLS de cliente para PostgreSQL/MySQL (PEM). En Unix se rechaza si el modo permite la lectura al grupo o a otros usuarios. Se lee al establecer la conexión y se rechaza para SQL Server con `DBP1015E`. |
| `--from-toml PATH` | Si se proporciona | Archivo TOML existente de dbwarp-blueprint, leído localmente para crear una presentación sin conexión a una base de datos. |
| `--from-parquet PATH` | Si se proporciona | Metadatos de Parquet y, solo con consentimiento explícito para el muestreo, filas decodificadas acotadas. |
| `--from-avro PATH` | Si se proporciona | Metadatos y registros del contenedor Avro; se recorre el contenedor para obtener el recuento de filas. |
| `--batch-manifest PATH` | Si se proporciona | Manifiesto y todas las rutas locales de entrada, credenciales, tokens y TLS que referencia. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Si se proporciona | TOML del paquete y archivos Blueprint relativos necesarios para enumerar, extraer o empaquetar. |
| terminal/consola de control (`/dev/tty` en sistemas tipo Unix) | Si no se proporciona ninguna fuente de contraseña | Solicitud con eco deshabilitado. |
| (solo durante la compilación) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` en versiones con dependencias incluidas, `vendor/*`, `vendor-crates/*` en paquetes sin conexión | Solo cuando se ejecuta `./build.sh` | Entradas de toolchain, procedencia del código y compilación Cargo |

La aplicación no tiene una ruta explícita para leer:
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- Ningún archivo `~/.ssh/*`
- `/etc/passwd` o `/etc/shadow` como entradas de Blueprint (las bibliotecas de
  identidad y autenticación de la plataforma pueden consultar datos de cuentas
  del sistema operativo)
- Ninguna variable de credenciales de base de datos salvo la indicada mediante `--password-env`,
  `--user-env` o `--azure-token-env`. Las compilaciones con Kerberos integrado
  también pueden hacer que la pila GSSAPI/Kerberos de la plataforma consulte su
  propia configuración, caché, keytab y variables de entorno. Las variables de
  idioma y presentación del terminal se describen abajo.

## Escrituras en el sistema de archivos

La herramienta escribe únicamente las salidas seleccionadas por el modo activo:

| Archivo | Cuándo | Contenido |
|---|---|---|
| `--out PATH` (valor predeterminado `./blueprint.toml`) | Ejecuciones de base de datos en vivo, Parquet, Avro, extracción de paquetes y empaquetado de paquetes | TOML Blueprint o de paquete empaquetado. No se escribe en modos de solo presentación, enumeración de paquetes, simulación, ayuda o versión. |
| `--deck PATH` | Solo si se especifica | Una presentación de PowerPoint (.pptx) que resume el Blueprint anonimizado. Se crea localmente a partir del mismo Blueprint en memoria o de la entrada `--from-toml`: sin lectura adicional de la base de datos, sin red y sin biblioteca de terceros. |
| `--audit-log PATH` | Solo si se especifica | Una copia sustituida atómicamente del registro de auditoría emitido en stderr; no se añade al contenido anterior. |
| `--out-dir DIR` | Modo por lotes que no sea una simulación | `bundle.toml`, directorios `blueprints/` y `audits/` por origen, un marcador de propiedad y `errors.txt` tras un error parcial. La publicación utiliza un directorio de preparación adyacente y un marcador de recuperación. |
| (solo durante la compilación) `./target/`, `./build/` | Solo cuando se ejecuta `./build.sh` | Salidas de compilación estándar de Cargo |

La aplicación no tiene una ruta explícita para escribir en:
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- ningún directorio temporal del sistema implícito (el usuario puede dirigir allí explícitamente una salida o un directorio por lotes)

## Variables de entorno leídas

La auditoría enumera solo las variables consultadas por DBWarp Blueprint. Si `--lang` no
selecciona un idioma compatible, la selección puede leer `DBWARP_BLUEPRINT_LANG`, `LC_ALL`,
`LC_MESSAGES` y `LANG`, en ese orden. La presentación del terminal puede leer `NO_COLOR`,
`TERM`, `COLORTERM` y `COLUMNS`; solo afectan a la presentación.

Cuando se especifica `--password-env VAR_NAME` o `--user-env VAR_NAME`,
la herramienta lee exactamente esa variable. No recurre a valores
predeterminados habituales como `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`,
`USER` o `LOGNAME`; esas alternativas no se han implementado
deliberadamente.

Las bibliotecas de base de datos, TLS, DNS y autenticación integrada de la
plataforma pueden consultar sus propias variables y configuración fuera de esta
lista de la aplicación. Use un rastreo del sistema operativo cuando la política
requiera un inventario completo del proceso y sus bibliotecas.

Cuando se ejecuta `./build.sh`, se leen `PINNED_RUST` (sustitución), `ALLOW_NETWORK`
(opción explícita para descargar rustup-init), `TARGET` (destino de compilación cruzada), además
de las variables estándar de cargo/rustup. La propia herramienta no lee ninguna
de ellas durante la ejecución.

## Registro de auditoría de cada ejecución

La herramienta emite un registro de auditoría en stderr en cada ejecución con un
formato de texto sin formato estable. Rediríjalo a un archivo con `2>audit.txt` o utilice
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
anonymization_key:   ephemeral-random
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
  requirements_complete: false
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
  - db.example:5432 (database-driver session; DNS may use the configured resolver)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - length policy balanced: declared capacities and index prefixes exact; sampled lengths relatively rounded
  - identifier ordering uses domain-separated HMAC-SHA256 with a fresh process-local key; labels intentionally vary between runs
  - the anonymization key and source identifiers are not written to the Blueprint
  - artifact summary stores bounded counts and external-prerequisite classes; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential entered through the Secret wrapper and its buffer is zeroized on drop; driver APIs may retain copies as documented under 'Driver-owned credential copies' in SECURITY.md

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

- Registra solo el número de selectores repetibles de captura en vivo `--schema`; sus valores aparecen en la comprobación previa interactiva, pero no se añaden a la auditoría. El URI de conexión existente, con los datos sensibles ocultos, sigue identificando la base de datos conectada, que también es el nombre del esquema en MySQL. Un Blueprint seleccionado se marca como `selection-limited` en `dataset_scope`.
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
- Registra una estimación determinista y desglosada por dimensiones de la fidelidad de Blueprint. La puntuación describe la cobertura de la evidencia capturada para estructura, dimensionamiento, estadísticas de columnas, relaciones y artefactos. No es un error medido frente a los datos fuente ni un intervalo de confianza estadístico. Las estadísticas de PostgreSQL señaladas como obsoletas o nunca analizadas reducen la puntuación de la dimensión de dimensionamiento y aparecen como limitaciones explícitas `table-statistics-stale` o `table-statistics-never-analyzed`. La actualidad de las estadísticas no puede compensar la falta de cobertura de los recuentos de filas. No se inventa evidencia de actualidad para los motores que utilizan contadores actualizados continuamente o cuya actualidad de las estadísticas no puede determinarse.
- Declara afirmaciones de confianza adecuadas al modo (nivel 1 frente a nivel 2).
- Usa un formato de texto estable, pero los valores pueden variar según el estado
  de la base de datos, el momento, las advertencias y la nueva clave de
  anonimización predeterminada. Para comparaciones aprobadas, reutilice un
  `--anonymization-key-file` protegido y fije `--generated-at`; los campos de
  tiempo siguen variando.

**Emisión condicional de afirmaciones de confianza.** La línea
"credential entered through the Secret wrapper..." solo se emite en ejecuciones
en las que realmente se leyó una credencial. Las rutas de error que terminan
antes de adquirir credenciales (errores al analizar la URI, rechazo de
contraseñas incrustadas en la URI, simulación, etc.) deliberadamente *no* emiten
esta línea: no hay nada que afirmar sobre una credencial que nunca se obtuvo.
Utilice la presencia o ausencia de la línea junto con
`auth.password_source` para saber si se ejercitó el tratamiento de credenciales
en una ejecución determinada.

**La auditoría se genera tanto en casos de éxito como de fallo**, incluyendo fallos en el análisis de comandos después del inicio. Los errores Help/version y los fallos que ocurren antes de que se pueda cargar el contrato de localización integrado no generan una auditoría completa. Si la herramienta falla a mitad de proceso (acceso denegado, error de red), el registro de auditoría aún se imprime en stderr (y en `--audit-log PATH` si se especifica) con `outcome: error: <stage>`, para que siempre tenga un registro de lo que se intentó antes del fallo. Ejemplo de línea de resultado de fallo:

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

- `--artifact-detail none` omite los catálogos del inventario de artefactos y las
  definiciones; la sonda de topología de solo recuento se sigue ejecutando.
- `summary` lee catálogos de objetos modelados, pero no el texto de las definiciones.
- `graph` también lee catálogos de dependencias, pero no el texto de las definiciones.
- `analyzed` también lee definiciones SQL/procedimentales disponibles en memoria de proceso acotada para el análisis léxico.

La auditoría registra el detalle solicitado, la visibilidad, los recuentos de objetos, dependencias y requisitos externos, y todos los indicadores de integridad. Cada operación de catálogo aparece en `database_operations_observed`. Un catálogo opcional fallido emite `DBP1410W`, aparece en `warnings` e impide una afirmación de integridad inexacta.

En modo de análisis, las definiciones se envuelven en un propietario que las anula, se eliminan y se reducen a bandas limitadas y tokens de características cerradas. El texto de la definición, los nombres de los objetos de origen, los puntos finales externos, los principales de los artefactos, las credenciales, el material de clave/certificado, los nombres de los paquetes/bibliotecas y los binarios nunca se escriben en Blueprint ni en el registro de auditoría. Los únicos nombres de principales exactos que se conservan son las tres identidades de sesión SQL Server en el bloque de auditoría `auth` explícito; nunca se escriben en los archivos Blueprint, deck o bundle. Los modos de gráfico y análisis requieren `--yes` porque la topología anónima aún puede identificar una aplicación.

- summary: solo recuentos acotados, sin identidades de objetos ni definiciones;
- graph: grafo anónimo de dependencias, sin definiciones;
- analyzed: definiciones leídas temporalmente, solo se conservan bandas acotadas.

Consulte [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) para la cobertura de familias de objetos y la interpretación de la integridad.

## Adiciones del nivel 2

Cuando la medición se acepta interactivamente, o sin interacción con `--measure-compression --yes`, la herramienta además:

- Para cada tabla que no se haya demostrado vacía, ejecuta una ruta de muestreo
  acotada específica del motor. PostgreSQL comienza con un porcentaje adaptativo
  de `TABLESAMPLE SYSTEM`, calculado a partir de la estimación del número de
  filas y del tamaño de muestra solicitado, con `LIMIT N`, y recurre a
  `LIMIT N` cuando es necesario;
  MySQL usa cuatro ventanas de rango acotadas sobre la clave primaria numérica
  cuando existe esa ruta de acceso segura; en caso contrario usa `LIMIT N`.
  SQL Server usa `TOP N`. Las rutas sesgadas marcan
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
controlador no expone siguen como `unknown`. El bloque `[compression]` por tabla del archivo de salida registra las proporciones. `--max-wall-secs` es un plazo
estricto para toda la captura en vivo, incluida conexión, catálogos, RTT y el muestreo del nivel 2.
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

Un archivo de plataforma descargado y una reproducción a partir del código
fuente son dos comprobaciones de confianza distintas. Verifique primero el
archivo y su ejecutable con la suma de comprobación publicada en la misma
versión. El archivo de plataforma es un paquete para el operador, no un árbol
de código fuente: su documentación y `verify.sh` son material de referencia
para una copia coincidente del código fuente. Obtenga la etiqueta exacta de la
versión del repositorio público, o utilice el archivo de código fuente con
dependencias de la versión, antes de intentar una auditoría o reconstrucción.
No espere poder reconstruir dentro de un archivo de plataforma.

Si desea *demostrar* que la herramienta solo hace lo documentado:

1. **Integridad de la descarga**: verifique el archivo de plataforma con la
   entrada correspondiente de `SHA256SUMS.txt` y, después, el ejecutable
   extraído con su archivo `*.binary.sha256`. Ambos archivos de sumas de
   comprobación deben proceder de la misma etiqueta de versión inmutable.
   Consulte [Descargar binarios](BINARIES.md).
2. **Auditoría del código fuente**: en la copia coincidente del código fuente o
   en el archivo de código fuente con dependencias, lea `src/secret.rs` y busque
   `\.expose\(\)` fuera de ese archivo. Si `rg` está instalado, el comando breve
   es:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   En caso contrario, utilice la herramienta de búsqueda de texto recursiva
   aprobada para su plataforma; `rg` no es un requisito de compilación ni de
   verificación.
   Las ubicaciones de llamada en producción entregan inmediatamente el `&str`
   expuesto al constructor de conexiones. MySQL también llama a `.to_string()`
   porque la API de `mysql_async` requiere `String`. Esa copia no se pone a cero
   y se transfiere a las opciones del controlador; puede sobrevivir a
   `OptsBuilder` durante la vida de las opciones o de la conexión. Eliminar el
   constructor no demuestra que se haya borrado la copia. Tier 1 y Tier 2 reutilizan la
   misma conexión MySQL. Consulte **Copias de credenciales propiedad del
   controlador** en SECURITY.md para ver la explicación completa.
3. **Compilación desde el código fuente**: `./build.sh`. Con el archivo de código
Ejecute `DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh`. Cada versión se construye dos veces y una discrepancia de bytes hace que la versión falle. Una comparación local solo es significativa con la misma revisión de origen, destino, características, cadena de herramientas Rust fijada, enlazador y opciones de compilación.
4. **Comparación con la versión**: desde la copia o el árbol coincidente del
   código fuente, ejecute `./verify.sh /path/to/extracted/dbwarp-blueprint`.
   Consulte **Reproducir un binario de versión** en BUILD.md para conocer el
   destino, las funciones, la cadena de herramientas, el enlazador, la época de
   fecha del código fuente y las opciones de compilación necesarias.
5. **Seguimiento durante la ejecución**: en Linux, ejecute con
   `strace -f -e trace=open,connect,read,write` en un entorno aislado. Si no
   dispone de `strace` o `rg`, utilice la herramienta equivalente de rastreo de
   archivos/red y búsqueda de texto recursiva aprobada para su plataforma.
   Compare el resultado con las listas anteriores.
6. **Seguimiento de red**: utilice `tcpdump` en el host. En una ejecución en
   vivo autenticada mediante contraseña, verifique la sesión de base de datos y
   el tráfico DNS esperado. Para la autenticación integrada, tenga en cuenta
   también el tráfico esperado hacia el KDC o el controlador de dominio. En el
   modo por lotes, reconcilie una sesión de base de datos por cada origen de
   base de datos.

Si alguno de estos elementos no coincide con lo que se documenta aquí, informe de la discrepancia a través del canal indicado en SECURITY.md e incluya el rastro más pequeño y seguro necesario para reproducirlo. No incluya credenciales, nombres identificativos ni resultados sensibles del controlador en un problema público.
