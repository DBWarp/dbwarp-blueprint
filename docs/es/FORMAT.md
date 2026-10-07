# Formato de archivo DBWarp Blueprint, versión 7.

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
# dbwarp-blueprint v7
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
| `schema_version` | int | Versión del formato. Actualmente `7`. Las versiones 1 a 6 siguen siendo legibles. |
| `generated_at` | Cadena ISO-8601. | Marca de tiempo UTC, con resolución de segundos, sin fracciones. Se puede "fijar" (***pinnable***) mediante la opción `--generated-at "2026-04-26T00:00:00Z"` de la interfaz de línea de comandos (CLI). Las capturas en vivo byte a byte también requieren la misma protección `--anonymization-key-file`, el estado de origen, las opciones y la versión del recolector. El registro de auditoría registra `generated_at_pin: ...` cada vez que se establece la opción, de modo que la "fijación" es visible para análisis forenses. Ninguna variable de entorno fija este valor. |
| `engine` | cadena | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"oracle"`, `"parquet"` o `"avro"`. `oracle` solo aparece en la salida de la vista previa de Oracle. |
| `engine_version` | cadena | Versión numérica del producto de la base de datos de origen; está vacía para las fuentes de archivos estructurados. Los banners de distribución están excluidos. |
| `source_kind` | cadena | Las fuentes de bases de datos utilizan los valores `"production"`, `"staging"`, `"scrubbed-replica"` o `"synthetic"` definidos por el operador. Las fuentes estructuradas utilizan `"parquet"` o `"avro"`. |
| `length_metadata` | cadena | Resumen: El marcador de resumen se mantiene para los lectores anteriores: `"hybrid-v2"`, `"exact"`, `"rounded"` o `"not-captured"`. Los tres campos a continuación son los oficiales. |
| `declared_length_fidelity` | string | `"exact"` para las capacidades de caracteres declaradas de PostgreSQL y para los modos MySQL equilibrado predeterminado y exacto; `"coarse-rounded-v1"` para privacidad estricta de MySQL; `"not-captured"` cuando no esté disponible. |
| `index_length_fidelity` | string | `"exact"` para prefijos de índice MySQL equilibrados predeterminados o exactos; `"rounded-down-v1"` para privacidad estricta; `"not-captured"` cuando no esté disponible. |
| `observed_length_fidelity` | string | `"relative-rounded-v2"` de forma predeterminada cuando se muestrea, `"exact"` en modo exacto, `"coarse-rounded-v1"` en modo estricto o `"not-sampled"`. La cobertura de muestreo sigue siendo un requisito independiente por columna. |
| `[totals]` | inline table | Recuentos agregados (consulte más abajo). |
| `[network]` | table | Evidencia opcional de la conexión cliente-base de datos y del RTT de consulta. |
| `[database_topology]` | tabla | Requerido para fuentes de bases de datos con esquema v6 y versiones más recientes. El esquema v7 utiliza el contrato de topología v2 y registra el alcance de cada recuento de miembros. No está presente en archivos estructurados. |
| `[dataset_scope]` | tabla | Requerido para cada Blueprint de la versión 6 y superior. Declara qué abarcan los totales y si la cobertura de tablas, filas y bytes es completa. |
| `[structure_scope]` | tabla | Requerido en la versión 7. Califica por separado la integridad del inventario de tablas, columnas, índices y relaciones. |
| `[source_environment]` | tabla | Requerido en los Blueprints de bases de datos de la versión 7 y prohibido para archivos estructurados. Contiene únicamente evidencia capacity/hosting general observada a través del punto de acceso de la base de datos o un proveedor explícito. |
| `[statistics_evidence]` | tabla | Requerido en la versión 7. Agregación exacta de las clasificaciones de conteo de filas por tabla, estadísticas del optimizador y evidencia de tamaño. |
| `[activity_snapshot]` | tabla | No escrito por DBWarp Blueprint 1.6. |
| `[tables.X]` | tables | Uno por tabla, con identificador anonimizado. |
| `[fk_edges]` | inline table | Grafo de claves foráneas entre tablas anonimizadas. Opcional. |
| `[artifact_inventory]` | tabla | Requerido en la versión 7, por lo que "no solicitado", "no aplicable", "ilegible" y un inventario verificado de cero objetos deben permanecer distintos. Contiene recuentos de objetos limitados y sin nombre, relaciones anónimas opcionales con tipos definidos, requisitos y un censo limitado del lenguaje. |

## `[totals]`

| Campo | Tipo | Precisión |
|---|---|---|
| `table_count` | int | exacta |
| `row_count` | int | suma de datos serializados por tabla `rows`; las estimaciones del catálogo están redondeadas, mientras que una lectura completa y delimitada verificada es exacta. |
| `table_bytes` | int | suma de los valores `table_bytes` redondeados de cada tabla |
| `index_bytes` | int | suma de los valores `index_bytes` redondeados de cada tabla |

Estos números no son automáticamente totales de todo el clúster. Siempre interprételos junto con `[dataset_scope]`. Una puerta de enlace o coordinador fragmentado puede exponer un catálogo que parece completo, pero que no contiene ninguno de los fragmentos subyacentes; las versiones de esquema v6 y v7 representan esa incertidumbre explícitamente, en lugar de tratar silenciosamente las estadísticas locales del catálogo como la verdad global.

`row_count` es una suma aritmética de los valores serializados por tabla, no una segunda medición sin redondear. Un recuento positivo conocido por debajo del primer nivel de privacidad se representa como `100` con `row_count_quality = "engine-estimate"` por tabla; por lo tanto, un conjunto de datos que contiene muchas tablas pequeñas puede tener un total conservadoramente alto. En ese caso, `dataset_scope.limitations` también contiene `row-counts-statistical`. Cero permanece reservado para indicar que la tabla de origen está vacía.

## `[database_topology]` (orígenes de bases de datos)

Este bloque registra solo hechos acotados visibles a través del endpoint de
base de datos conectado. Nunca almacena nombres de nodos o hosts, direcciones
IP, nombres de clúster o canal de replicación, identificadores de servidor ni
endpoints.

| Campo | Valores / regla |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` en el esquema v6; `dbwarp-blueprint-topology/v2` en v7. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` o `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby`, o `unknown`. |
| `visibility` | `full`, `partial` o `unknown`; describe la evidencia de topología, no la corrección de los datos. |
| `member_count` | Número de miembros visibles mediante consultas de evidencia correctas. `0` significa desconocido, nunca cero miembros. |
| `member_count_scope` | Solo para la versión V7: `deployment`, `visible-subset`, `connected-member` o `unknown`. Para tener visibilidad completa de la topología, se requiere `deployment`; `connected-member` requiere un recuento de uno. |
| `identifiers_redacted` | Debe ser `true`. |
| `role_counts` | Recuentos opcionales por token cerrado de rol. La visibilidad completa exige que sumen `member_count`. |
| `features` | Tokens cerrados ordenados como `citus`, formularios de MySQL replication/cluster, `postgresql-streaming-replication`, `sqlserver-availability-group`, `oracle-non-cdb`, `oracle-cdb`, `oracle-pdb`, `oracle-rac`, `oracle-data-guard` o `vitess`. |
| `catalogs_read` | Etiquetas cerradas y ordenadas de catálogos de topología leídos correctamente. |
| `catalogs_unreadable` | Etiquetas cerradas y ordenadas de catálogos de topología no legibles. Cualquier entrada impide afirmar visibilidad completa. |
| `catalogs_not_applicable` | Solo para la versión V7. Las etiquetas cerradas ordenadas demostraron ser inaplicables a esta fuente. Es distinta de los conjuntos legibles e ilegibles. |

Un punto final ordinario puede informar legítimamente `deployment = "unknown"` mientras sigue informando estadísticas locales completas de tablas con copia completa. Blueprint no deduce que un servidor corriente sea de nodo único simplemente porque no se observó ninguna característica de clúster.

## `[dataset_scope]` (esquema v6 y versiones más recientes)

Este bloque califica cada total de tamaño de forma independiente. No trate los totales como cifras del conjunto de datos completo cuando cualquier dimensión de integridad requerida sea `incomplete` o `unknown`.

| Campo | Valores / regla |
|---|---|
| `contract` | Siempre `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` o `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` o `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` o `unknown`. |
| `size_completeness` | `complete`, `incomplete` o `unknown`. |
| `row_count_method` | Un token de procedencia cerrado, como `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter`, `oracle-table-statistics` o `oracle-segment-statistics`. `bounded-complete-read` y `mixed-catalog-and-bounded-read` identifican totales recuperados de una lectura de Nivel 2 completa y verificada, ya sea solos o junto con los conteos del catálogo. Oracle utiliza `not-applicable` junto con el mismo método de tamaño solo cuando un inventario no vacío no tiene tablas en la población de totales de copia. `distributed-aggregate` se acepta como entrada, pero no se escribe en esta versión. |
| `size_method` | Un token de procedencia cerrado, como `postgres-local-relation-size`, `citus-distributed-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `oracle-segment-bytes`, `oracle-table-logical-estimate`, `mixed` o `not-applicable`. Oracle utiliza `mixed` cuando las tablas incluidas combinan contadores de segmentos atribuidos con estimaciones lógicas etiquetadas. Utiliza `not-applicable` solo cuando un inventario no vacío no tiene tablas en la población total de copias, de modo que un total cero completo no afirme falsamente un método de medición. `distributed-aggregate` se acepta como entrada, pero no se escribe en esta versión. |
| `limitations` | Motivos cerrados y ordenados de cobertura incompleta o desconocida. Se requiere al menos uno salvo que todas las dimensiones estén completas. |

`selection-limited` significa que los totales y las declaraciones de integridad cubren exactamente los esquemas solicitados mediante el selector repetible en vivo `--schema`; no afirman cubrir toda la base de datos conectada. Si se omite `--schema`, se conserva la captura de todos los esquemas visibles.

Un esquema seleccionado y legible puede contener legítimamente solo objetos que no son tablas y se mantiene en los inventarios correspondientes. Sin embargo, cuando la captura completa no contiene tablas en absoluto, el recolector no debe publicar un conjunto de datos vacío definitivo: la integridad de las tablas, las filas y el tamaño sigue siendo incompleta y `table-inventory-visibility-unknown` registra el límite conservador.

`row-count-evidence-incomplete` y `size-evidence-incomplete` significan que al menos una tabla incluida carecía del valor de catálogo correspondiente. El total numérico es entonces la suma de las contribuciones conocidas, no una afirmación de que una tabla no disponible contenía cero filas o bytes. Las estadísticas por tabla indican qué registros no están disponibles.

Para Oracle, `oracle-segment-bytes` es la evidencia preferida y exacta del tamaño asignado. Si no se puede atribuir el almacenamiento para una tabla a partir de `DBA_SEGMENTS`—por ejemplo, una tabla agrupada o una tabla organizada por índice cuyo catálogo de índice no está disponible—el recolector puede emitir `oracle-table-logical-estimate` utilizando los valores `DBA_TABLES.NUM_ROWS * AVG_ROW_LEN` ya disponibles. La evidencia de la tabla entonces contiene `size_quality = "engine-estimate"`, `size_scope = "table-only"`, y `size_accounting = "logical-estimate"`, `size_visibility = "partial"`, y la integridad del tamaño del conjunto de datos es `incomplete`; no afirma bytes de LOB ni de índice. Un catálogo de refinamiento faltante nunca causa que se descarten bytes ya atribuidos a esa tabla lógica: la contribución medida permanece `oracle-segment-bytes`, con visibilidad parcial y cobertura agregada incompleta. El almacenamiento compartido o organizado por índice que no se puede atribuir no se publica como un cero exacto. Una tabla de Oracle con un índice de dominio también utiliza visibilidad parcial y alcance desconocido porque las implementaciones de Text, Spatial y otros dominios pueden almacenar bytes en objetos secundarios fuera del inventario de la tabla de usuario emitido. La alternativa lógica es la evidencia de dimensionamiento de copia en lugar de un contador de bytes asignados. Puede sobreestimar la asignación actual cuando los recuentos de filas del optimizador permanecen obsoletos después de que se liberó el almacenamiento (por ejemplo, después de `TRUNCATE ... DROP STORAGE`), y su origen de la estimación debe conservarse. No se requiere ningún permiso `DBA_TABLESPACES` para esta alternativa.

Para el almacenamiento de índices en Oracle, la lectura completa del catálogo de índices representa el límite lógico. Una fila `DBA_SEGMENTS` posterior que no tenga una identidad de índice coincidente se encuentra fuera de esa población y no se asigna a una tabla arbitraria. Un índice que existía en el límite pero que carece de su segmento esperado no proporciona visibilidad completa de su tamaño para su tabla.

`logical-partition-root-unmeasured` es una evidencia específica de PostgreSQL que indica que una partición lógica raíz incluida contribuye deliberadamente ni filas ni bytes, porque esos valores se encuentran en sus hojas físicas. A diferencia de una `row-count-evidence-incomplete` estadística que puede corregirse, una lectura completa de otra tabla no puede restaurar la integridad del conjunto de datos mientras dicha raíz permanezca en el inventario seleccionado.

`table-inventory-visibility-unknown` significa que el recolector no pudo leer la clasificación propiedad del motor, que es necesaria para separar los objetos de usuario de los objetos de soporte. Es posible que los registros visibles aún estén presentes, pero se retira la integridad de las tablas, las filas y el tamaño, en lugar de tratar ese subconjunto como el conjunto completo.

`catalog-capture-truncated` significa que una sesión de catálogo de una sola fuente se detuvo antes de que se leyera cada familia o esquema previsto. Los registros que ya se han comprobado que están completos aún pueden emitirse, pero ninguna afirmación sobre la integridad del conjunto de datos o la estructura puede extenderse al resto no leído.

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

## `[structure_scope]` (esquema v7)

Este bloque permite distinguir un catálogo vacío y verificado de un catálogo que fue filtrado, no se pudo leer o no se inspeccionó.

| Campo. | Valores / regla. |
|---|---|
| `contract` | Siempre `dbwarp-blueprint-structure-scope/v1`. |
| `visibility` | `full`, `privilege-filtered` o `unknown`. La integridad se limita a los esquemas seleccionados y al ámbito de privilegios visible; no es una afirmación de visibilidad ilimitada de la base de datos. |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | Independientemente de `complete`, `incomplete` o `unknown`. Las familias dependientes no pueden indicar que están completas si su familia principal requerida está incompleta. |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | Etiquetas de catálogo cerradas y disjuntas, ordenadas. `catalogs_read` registra evidencia positiva de lectura; en una captura con múltiples propietarios, puede retener un catálogo incluso si al menos una lectura del propietario previsto tuvo éxito, aunque otra no. `catalogs_unreadable` significa que no sobrevivió ninguna lectura positiva. Una familia completa requiere su catálogo específico del motor en `catalogs_read`, que cada consulta de la familia prevista se haya completado, y que no haya ninguna laguna afectada por objeto. |
| `limitations` | Razones cerradas y ordenadas, como `selection-limited`, `metadata-visibility-privilege-filtered` o `table-kinds-not-inventoried`. La evidencia parcial o desconocida requiere una razón. |

Los selectores de esquema forman parte del alcance: `complete` significa completo para los esquemas seleccionados y resueltos, pero no necesariamente todos los esquemas del servicio. Un selector que no resuelve ningún esquema es un error y no debe convertirse en un Blueprint vacío completo.

`catalog-capture-truncated` tiene el mismo significado en la evidencia estructural: los registros publicados de tablas y columnas son el prefijo o subconjunto de propietario verificados, no una afirmación de que se haya completado el trabajo del catálogo previsto restante. Un catálogo que no se intentó en esa instancia no aparece en ninguno de los tres conjuntos de catálogos; no debe ser etiquetado como ilegible o no aplicable.

Para una lectura con múltiples propietarios, `index-inventory-unavailable` o `relationship-inventory-unavailable` pueden acompañar a un catálogo en `catalogs_read`: la etiqueta del catálogo conserva la evidencia positiva del propietario exitoso, mientras que el campo de completitud y el registro de limitaciones indican que no se observó toda la población seleccionada. Las limitaciones por tabla identifican los objetos emitidos con una brecha de representatividad; no reemplazan la evidencia del estado de la consulta para un propietario denegado o que no intentó, que no emitió ninguna tabla.

Los registros de Oracle `oracle-identity-columns` y `oracle-constraint-columns` se almacenan por separado de sus catálogos de columnas y restricciones principales. Su presencia o ausencia describe la generación de identidad opcional y la evidencia de la clave de relación; no deben interpretarse como una afirmación de que el catálogo principal no era legible.

## `[source_environment]` (orígenes de bases de datos con esquema v7)

Este bloque nunca describe la estación de trabajo que ejecuta `dbwarp-blueprint`. `collector_machine_excluded` debe ser `true`. La evidencia de capacidad proviene únicamente del punto de conexión de la base de datos conectado o de un proveedor, orquestador u operador autorizado.

| Campo. | Valores / regla. |
|---|---|
| `contract` | Siempre `dbwarp-blueprint-source-environment/v1`. |
| `evidence_origin` | `database-endpoint`, `provider-api`, `orchestrator-api`, `operator-attested`, `mixed`, o `none`. |
| `hosting_model` | `managed-service`, `self-managed`, `orchestrated` o `unknown`. |
| `infrastructure_location` | `cloud`, `on-premises`, `hybrid` o `unknown`. |
| `capacity_scope` | `connected-instance`, `database-resource`, `cluster-aggregate`, `member-subset`, o `unknown`. |
| `capacity_visibility` | `capacity_visibility` puede ser `full`, `partial`, `unknown` o `not-requested`. `not-requested` requiere bandas, bases y alcance de capacidad desconocidos, y no tiene un catálogo de capacidad clasificado. Puede seguir presente una clasificación ajena a la capacidad, como la edición de SQL Server. |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus`, o `unknown`. |
| `cpu_capacity_basis` | `logical-cpu-limit`, `database-resource-limit`, `operating-system-visible`, `physical-host`, o `unknown`. `operating-system-visible` no afirma que la asignación de una máquina virtual, un contenedor o un servicio administrado sea el host físico subyacente. |
| `memory_capacity_band` | Bandas amplias desde `under-2-gib` hasta `512-gib-plus`, o `unknown`. |
| `memory_capacity_basis` | `database-buffer-cache`, `database-resource-limit`, `operating-system-visible`, `physical-host`, o `unknown`. `operating-system-visible` es la base conservadora para un motor DMV cuyo valor puede describir un entorno virtual o un contenedor en lugar de hardware físico. Una banda `database-buffer-cache` es la asignación de caché configurada y, por lo tanto, solo un límite inferior de la memoria total del origen; nunca debe representarse como la capacidad del host sin su base. |
| `member_capacity_uniform` | Opcional observed/attested, tipo booleano; la omisión significa desconocido. |
| `features` | Hechos cerrados ordenados como `autoscaling`, `burstable`, `container-limits-visible`, `database-resource-governed`, `serverless` o `shared-host`. |
| `limitations` | Limitaciones de procedencia cerradas y ordenadas. `oracle-client-version-mismatch` o `oracle-client-version-unreadable` indican que la versión del cliente Oracle SQL*Plus no pudo atestiguarse por completo. `oracle-client-version-below-tested-floor` indica un cliente atestiguado anterior a 12.1, el umbral de comparación codificado en este contrato. La captura del catálogo continúa porque la procedencia del banner del cliente no determina la estructura de la base de datos. |
| conjuntos de catálogo | Evidencia ordenada y separada de los catálogos de entorno de origen intentados, incluyendo la clasificación de la edición de SQL Server, incluso cuando su DMV de capacidad opcional no es legible. |

La capacidad desconocida no es cero capacidad. Una conexión remota no autoriza la lectura del CPU o la memoria del host del recolector y su posterior etiquetado como capacidad del servidor.

Para Oracle, un catálogo de capacidad que se completó solo para una parte del conjunto de consultas previsto sigue siendo una evidencia `catalogs_read` positiva, pero sus valores se omiten y `capacity_visibility` es `unknown`. Las filas parciales no deben presentarse como un límite de CPU o memoria a nivel de toda la infraestructura.

Los ajustes de Oracle SQL*Plus se seleccionan a partir de la versión numérica de la sesión en ejecución cuando es legible, en su defecto a partir del banner del ejecutable y, por último, a partir de un protocolo conservador que no selecciona ni `ROWLIMIT` ni el marcado CSV como función de salida. Ambos pasos de configuración siguen intentando borrar un `ROWLIMIT` y un modo CSV heredados; el diagnóstico de opción desconocida de un cliente antiguo solo se tolera dentro de la ventana de restablecimiento delimitada. Una incompatibilidad de versión, una atestación parcial, un fallo de análisis o un cliente atestiguado anterior a 12.1 solo debilitan la procedencia. Nunca bloquean la captura del catálogo.

## `[statistics_evidence]` y `[tables.<id>.statistics]` (esquema v7)

Cada tabla de la versión 7 tiene un bloque de estadísticas. El bloque de nivel superior contiene recuentos exactos por `statistics_state`, `row_count_quality` y `size_quality`; cada mapa debe cubrir cada tabla y ser exactamente igual a las clasificaciones a nivel de tabla. La visibilidad agregada es `full` solo para una población total de copias no vacía cuando cada tabla contada tiene visibilidad de tamaño completo, evidencia de filas conocida y un estado de estadísticas clasificado, y ningún catálogo de estadísticas es ilegible. Los objetos externos, temporales y derivados, que se excluyen deliberadamente, permanecen inventariados; su indisponibilidad de filas y tamaño, impulsada por políticas, no reduce la visibilidad de esa población de copias, pero un estado de estadísticas no clasificado sí lo hace. Cuando se excluye cada tabla, la visibilidad agregada es `unknown` con `statistics-visibility-unknown`; una población vacía no debe obtener `full` de forma vacua. `catalog-capture-truncated` registra que el trabajo previsto en el catálogo de estadísticas se detuvo antes de que se alcanzara a cada propietario, manteniendo cualquier evidencia positiva de lectura del catálogo que ya se obtuvo. La misma regla de evidencia positiva se aplica cuando una lectura de propietario tiene éxito y otra se niega: el catálogo permanece en `catalogs_read`, mientras que `statistics-partial` y la visibilidad agregada registran que la población seleccionada no se observó completamente.

Los campos a nivel de tabla son:

| Campo. | Valores / regla. |
|---|---|
| `row_count_method` | Engine/version-aware método de catálogo, `bounded-complete-read` cuando una instrucción de Nivel 2 enumeró de forma segura la tabla visible, un contador de archivo estructurado, o `unknown`; la captura ordinaria no recurre silenciosamente a `COUNT(*)`. |
| `row_count_quality` | `exact-counter`, `exact-read`, `engine-counter`, `engine-estimate`, `cached-engine-estimate`, `sample-extrapolation`, `unavailable`, o `unknown`. Un contador de SQL Server conocido como positivo, que se encuentra por debajo del primer "bucket" de privacidad con valor distinto de cero, utiliza `engine-estimate` después de serializar `rows = 100`; esto distingue el "bucket" de privacidad tanto de un contador exacto como de un valor cero medido. |
| `statistics_state` | `current`, `possibly-stale`, `known-stale`, `never-analyzed`, `locked`, `user-supplied`, `not-applicable`, o `unknown`. |
| `refresh_age_band` | `under-1h`, `1h-1d`, `1-7d`, `1-4w`, `1-3m`, `3m-plus`, `unknown`, o `not-applicable`. |
| `modification_ratio_band` | `none`, `under-1pct`, `1-5pct`, `5-10pct`, `10-20pct`, `20-50pct`, `over-50pct`, `unknown`, o `not-applicable`. |
| `sample_fraction_band` | `full`, `75-99pct`, `50-74pct`, `25-49pct`, `under-25pct`, `unknown`, o `not-applicable`. |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object`, o `unknown`. |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | Describa por separado de dónde proviene el tamaño, si es un contador o una estimación, si incluye el almacenamiento LOB/index, si es un tamaño asignado o lógico, y si la visibilidad es completa, parcial, no disponible o desconocida. |

El bloque de estadísticas de nivel superior utiliza `visibility = "full"`, `"partial"` o `"unknown"` para la población total de copias no vacías descrita anteriormente. Nunca utiliza solo objetos excluidos para obtener visibilidad de `full`.

La evidencia de Oracle `oracle-segment-bytes` es válida solo con `exact-counter`, `allocated-segment`, visibilidad completa o parcial, y un `segment_state` que demuestre que el censo del segmento se atribuyó (`created`, `deferred`, `mixed` o `mixed-table-and-index`). Un fallback lógico utiliza `oracle-table-logical-estimate`, `engine-estimate`, `logical-estimate`, visibilidad parcial y un estado de segmento no disponible. Esto evita que una clase de almacenamiento no atribuida se convierta en un cero medido. El estado se deriva de la evidencia del catálogo atribuida antes del redondeo de privacidad. `created` puede, por lo tanto, acompañar a bytes de tabla serializados a cero cuando un contador de tabla sin procesar positivo conocido cae por debajo del primer cubo de bytes. La evidencia parcial medida de Oracle utiliza `size_scope = "unknown"`: los bytes atribuidos permanecen exactos, pero la falta de un LOB, almacenamiento anidado o mapeo de índice significa que el recolector no puede afirmar honestamente el alcance completo de table/LOB/index. Para Oracle, `mixed` significa que una asignación de índice positiva atribuida se suprimió a `index_bytes = 0` serializado mediante redondeo; esto mantiene el cero distinto de una tabla para la cual el censo del segmento no encontró ninguna asignación de índice. `mixed-table-and-index` significa que tanto las asignaciones de tabla como de índice fueron positivas antes del redondeo y que ambos contadores serializados son cero, preservando ambos hechos sin revelar los valores de bytes de subcubo. `deferred` significa que los contadores sin procesar atribuidos fueron cero y requiere que ambos valores de bytes serializados sean cero. Utilice el estado para distinguir una asignación de subcubo redondeada de un almacenamiento que se ha demostrado que no está materializado.

El bloque de nivel superior también registra catálogos disjuntos ordenados y limitaciones cerradas. Un valor de `rows` o `table_bytes` puede utilizarse como un cero observado solo con la evidencia de respaldo quality/visibility; no ignore el bloque de procedencia. Una tabla contada cuya calidad de fila o tamaño es `unavailable` o `unknown` aleja la integridad del conjunto de datos correspondiente de `complete`; el validador rechaza un marcador de posición numérico presentado como cobertura completa. En particular, una tabla de PostgreSQL que no tiene estadísticas del optimizador ni una lectura completa y delimitada comprobada tiene un volumen de filas desconocido, no un cero medido.

Oracle Basic omite `check_count` cuando el diccionario no puede distinguir una restricción `NOT NULL` declarada de una restricción `CHECK` explícita y textualmente idéntica. No infiere a partir del nombre de una restricción generada ni de la nulabilidad actual de la columna. Otras tablas cuyas filas de restricción son inequívocas aún pueden contener un recuento exacto.

## `[activity_snapshot]`

DBWarp Blueprint 1.6 no escribe este bloque.

## `[network]` (opcional)

El tiempo de ida y vuelta desde la máquina que ejecuta el recolector hasta su base de datos. Esto no es el tiempo de ida y vuelta entre la fuente y el destino de la migración.

La sonda se ejecuta después de establecer la conexión y antes de consultar el
catálogo, por lo que los tiempos no quedan distorsionados por el calentamiento
de la caché de consultas. Ejecuta **5× `SELECT 1`** y emite la latencia mediana.
Cada `SELECT 1` devuelve la constante entera 1; esta sonda nunca lee datos de
filas.

Ausente cuando `--no-rtt-probe` se utiliza o cuando la propia prueba falla durante su ejecución (se registra como una advertencia no fatal en stderr y en el registro de auditoría; el archivo Blueprint se genera de todos modos, sin el bloque).

| Campo | Tipo | Precisión |
|---|---|---|
| `sample_count` | int | exacta (siempre 5 en v1) |
| `connect_total_ms` | int | tiempo de reloj total desde el inicio de la conexión TCP hasta que la sesión autenticada está lista, en milisegundos. Incluye el protocolo de enlace TCP, el protocolo de enlace TLS (cuando corresponde) y el desafío/respuesta de autenticación. Redondeado al milisegundo más cercano. Normalmente equivale a 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | latencia mediana de una sola ida y vuelta de las 5 muestras `SELECT 1`, en milisegundos. Redondeada al milisegundo más cercano. El nivel natural de ruido de la red (≥ 1 ms en la práctica) es mayor que la granularidad del redondeo, por lo que se elimina cualquier canal encubierto de bits bajos sin perder precisión útil. Los valores de LAN inferiores a un milisegundo se reducen a 0 o 1. |
| `query_rtt_ms_p95` | int | percentil 95 de las 5 muestras calculado mediante el método del rango más próximo (la observación más lenta), en milisegundos. Redondeado al milisegundo más cercano. Úselo con p50 para detectar picos breves de latencia; cinco muestras solo sirven como orientación y no constituyen una prueba de rendimiento de una carga de trabajo. |

Las cinco consultas de sondeo aparecen en el registro de auditoría como **una sola entrada resumida** (no cinco filas separadas), con la etiqueta `5x SELECT 1 (RTT probe; constant integer 1, no row data)`. Esto corresponde al principio de confianza de que no se lee ningún contenido de fila.

## `[tables.<id>]`

El identificador es `table-NNN`, donde `NNN` es el índice de 1 en una ordenación HMAC-SHA256 separada por dominios del nombre del esquema y la tabla. La clave predeterminada se genera para el proceso y nunca se emite. Pasar el mismo `--anonymization-key-file` protegido preserva el orden en las comparaciones aprobadas. El esquema v7 requiere el conjunto completo y denso de ordinales desde `table-001` hasta el número de tablas emitidas (el ancho crece naturalmente en `table-1000`); los sufijos omitidos, cero, no decimales o derivados de la fuente son inválidos.

| Campo | Tipo | Precisión / valores |
|---|---|---|
| `rows` | int | Las estimaciones del catálogo se redondean: al múltiplo de 100 más cercano (≤10k), al múltiplo de 1000 más cercano (≤1M) o al múltiplo de 10000 más cercano (>1M). Una estimación positiva conocida que, de otro modo, se redondearía a cero, utiliza el primer valor distinto de cero (`100`); el cero está reservado para un catálogo cero o evidencia no disponible identificada por la calidad de las estadísticas adyacentes. Cuando una lectura limitada de Nivel 2 demuestra que ha enumerado la tabla visible completa, `rows` es el número exacto ya revelado por la muestra exacta `sample_rows`; esto evita contar contradicciones por tabla, cardinalidad y agregados sin agregar un nuevo canal. |
| `table_bytes` | int | redondeado: al 1KiB, 1MiB o 100MiB más cercano según la magnitud |
| `index_bytes` | int | redondeado: igual que `table_bytes` |
| `schema` | cadena | Un identificador anonimizado `schema-A`, `schema-B`, ..., `schema-AA`. El esquema v7 requiere un conjunto ordinal alfabético denso para cada esquema referenciado por una tabla emitida o un artefacto graph/analyzed; por lo tanto, se conserva un esquema seleccionado que contiene solo objetos que no son tablas. |
| `object_kind` | cadena | La versión V7 requiere un token de cierre: `ordinary-table`, `materialized-view`, `external-table`, `temporary-table`, `nested-table` o `object-table`. La identidad del objeto es independiente del almacenamiento físico y la partición. |
| `storage_organization` | cadena | La versión V7 requiere un token de cierre: `heap`, `index-organized`, `clustered`, `external` o `unknown`. `external` solo es válido para `object_kind = "external-table"`. |
| `partitioning` | cadena | La versión V7 requiere un token de cierre: `none`, `range`, `list`, `hash`, `interval`, `reference`, `composite`, `system`, `key`, `linear-hash`, `linear-key` o `unknown`. |
| `segment_state` | cadena | La versión V7 requiere un token de cierre: `created`, `deferred`, `mixed`, `mixed-table-and-index`, `unavailable` o `unknown`. Esto separa los objetos que solo contienen metadatos de los que tienen almacenamiento materializado. Es una evidencia categórica establecida antes del redondeo de bytes, por lo que `created` puede acompañar a cero bytes de tabla serializados para una asignación positiva de sub-bucket. Con la evidencia del contador de segmentos de Oracle, `mixed` registra una asignación de índice atribuida positiva cuya `index_bytes` serializada se redondea a cero; `mixed-table-and-index` registra que ambas asignaciones sin procesar fueron positivas, mientras que ambos contadores serializados se redondearon a cero. |
| `parent_table`, `child_tables` | cadena / array | Enlaces anónimos opcionales y recíprocos entre tablas para objetos anidados, particionados u otros contenidos. Los ID de los elementos secundarios están ordenados y son únicos; el grafo padre debe ser acíclico. |
| `table_features` | arreglo. | Tokens cerrados ordenados: `graph-edge`, `graph-node`, `memory-optimized`, `temporal-current`, o `temporal-history`. |
| `unlogged` | bool | Observación opcional del estado registrado de PostgreSQL. Se omite cuando no se captura; `false` explícito significa que el catálogo confirmó que la tabla está registrada. |
| `partition_count` | int | El número exacto de particiones físicas incluidas, requerido cuando `partitioning` especifica una estrategia de particionamiento conocida. PostgreSQL informa las particiones hoja recursivas y excluye las particiones que se encuentran fuera de los esquemas resueltos según `selection-limited`. Las tablas compuestas de MySQL cuentan las subparticiones porque estas son sus particiones físicas; por ejemplo, cuatro particiones de nivel superior con ocho subparticiones cada una informan `32`. Cero solo es válido para una raíz de partición lógica con `segment_state = "unavailable"` y sin particiones hoja incluidas. |
| `partition_key_cols` | arreglo de enteros. | Ordinales completos de las columnas de una clave de partición simple. Se omiten para una clave basada completamente o parcialmente en expresiones, o cuando la información del catálogo no está disponible; una lista parcial de ordinales y las expresiones de la clave nunca se serializan. |
| `partition_rows_max` | int | Estimación opcional del número máximo de filas en la hoja más grande. Para los totales de tablas con una precisión estimada, un valor conocido y positivo utiliza el primer bloque de filas no nulo, limitado por `rows`. Con una población de tabla con lectura exacta, la estimación máxima de la hoja cuyo bloque de privacidad sería cero o excedería esa población exacta se omite como irrepresentable en lugar de ser ajustada a un valor falso. Cuando está presente, no puede ser cero mientras `rows` sea positivo o exceda `rows`. |
| `temporal_history` | cadena | El identificador de tabla anónimo de la tabla de historial temporal asociada es obligatorio con la función `temporal-current`, a menos que esa tabla contenga un token `table_limitations` aplicable por objeto. La selección a nivel de captura por sí sola nunca elimina la vinculación. |
| `table_limitations` | arreglo. | Evidencia cerrada y ordenada por objeto. `table-classification-unavailable` identifica una tabla cuyos datos de entrada de tipo de objeto estaban incompletos. `column-inventory-unavailable` identifica una tabla con uno o más registros de columna faltantes o ilegibles; `dependent-structure-suppressed` indica que ni el índice ni la estructura de relación para esa tabla se pueden considerar completos porque falta una columna. `index-inventory-unavailable` y `relationship-inventory-unavailable` reducen una brecha de solo refinamiento a la familia dependiente afectada sin retirar el inventario de columnas requerido. Cualquier índice, clave de partición o relación que haga referencia a una columna emitida ausente se omite en lugar de permitir que invalide toda la captura. `relationship-target-outside-selected-scope` registra que al menos una clave externa declarada en esta tabla apunta a un objeto fuera del ámbito del esquema seleccionado resuelto; solo es válida en una captura `selection-limited`. `relationship-target-visibility-unknown` registra que el catálogo reveló un objetivo de clave externa que no se pudo resolver en el inventario visible; la integridad de la relación debe ser incompleta. `row-security-filter-active` registra un predicado de filtro SQL Server visible y habilitado. `row-security-visibility-unknown` registra que no se pudo demostrar la visibilidad completa del catálogo de políticas de seguridad de SQL Server, por lo que se suprime el muestreo de Nivel 2 en lugar de tratar un subconjunto potencialmente filtrado como la población de la tabla. `temporal-history-outside-selected-scope` solo es válida para una tabla temporal actual no vinculada en una captura `selection-limited` después de que el recolector resolviera el esquema de historial fuera de la selección. `temporal-history-visibility-unknown` registra que el catálogo reveló un ID de objeto de historial pero no suficientes metadatos para resolverlo. |
| `counted_in_totals` | bool | Omitido significa incluido. Una `external-table`, `materialized-view`, `temporary-table` o tabla que contiene `memory-optimized` requiere `false` explícito, excluyendo datos externos, derivados, específicos de sesión o actualmente no medidos de `table_count`, `row_count`, `table_bytes` y `index_bytes`. La evidencia específica de cada objeto sigue estando disponible para la planificación de la recreación, sin presentar valores no disponibles como totales medidos. Ningún otro valor explícito es canónico. |
| `check_count` | int | Opcional, recuento exacto de restricciones CHECK estructurales. Si se omite, significa desconocido; `0` significa que el catálogo relevante no tiene ninguna. |
| `has_clustered_index` | bool | siempre `false` para PostgreSQL |
| `[tables.<id>.statistics]` | subtabla | Se requiere la información de versión 7 para el recuento de filas, el estado de las estadísticas del optimizador y la evidencia del tamaño. El campo `stats_freshness` de la versión 6 solo se acepta al leer archivos más antiguos y nunca se emite en la versión 7. |
| `[tables.<id>.cols.<cid>]` | sub-tables | uno por columna |
| `[tables.<id>.idxs.<iid>]` | sub-tables | uno por índice |
| `[tables.<id>.compression]` | sub-table | solo si es de nivel 2 |

## `[tables.<id>.cols.<cid>]`

El identificador es `col-N`, donde `N` es el orden natural de los atributos de la columna (indexado desde 1, conservando el orden en disco). Es estable entre ejecuciones. En el esquema v7, el sufijo decimal debe ser exactamente igual a `ordinal`; los ceros, las representaciones con ceros iniciales y las etiquetas derivadas de la fuente no son válidos. Los motores de origen pueden mantener espacios en los ordinales físicos de las columnas después de que se elimina una columna.

| Campo | Tipo | Notas |
|---|---|---|
| `ordinal` | int | el mismo N que en el identificador |
| `type` | string | familia de tipos normalizada, como `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` o `"user-defined"`. No se emiten nombres reales de dominios, enumeraciones, alias, tipos compuestos ni tipos definidos por el usuario. |
| `nullable` | bool |  |
| `value_source` | string | Token cerrado opcional del esquema v6: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` o `rowversion`. Se omite para un valor ordinario o evidencia desconocida. |
| `has_default` | bool | Observación opcional del catálogo en el esquema v6. La omisión significa desconocido; `false` confirma que no hay valor predeterminado. |
| `default_kind` | string | Clasificación opcional `constant`, `function` o `expression` en el esquema v6, válida solo con `has_default = true`. Nunca se serializan el texto ni los literales. |
| `default_on_null` | bool | V7: Observación opcional del catálogo de origen para Oracle `DEFAULT ON NULL`; válida solo cuando se especifica un valor predeterminado. "Omitido" significa que no se observó. |
| `type_kind` | string | Token cerrado opcional del esquema v6: `enum`, `set`, `domain`, `composite`, `array`, `range` o `alias`. Se omite para un tipo base o evidencia desconocida. |
| `member_count` | int | Número estructural exacto y positivo de miembros en el esquema v6, obligatorio solo para `enum` y `set`. Nunca se serializan sus nombres. |
| `domain_has_check` | bool | Observación opcional del CHECK de un dominio en el esquema v6, válida solo con `type_kind = "domain"`. |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | bool | Observaciones opcionales del catálogo. `invisible` es distinto de una columna oculta creada por el motor. Omitido significa desconocido; explícito `false` significa que el catálogo demostró que la propiedad está ausente. |
| `has_check` | bool | Observación opcional de un CHECK de una sola columna en el esquema v6. Cada `true` está cubierto por `check_count` de la tabla. |
| `null_fraction` | flotante | Opcionalmente, se observa la fracción de valores nulos desde `0.0` hasta `1.0`. Cuando la cardinalidad está presente, se deriva de los conteos públicos redondeados para la privacidad de ese bloque; de lo contrario, se redondea de forma independiente. No se conserva ningún mapa de bits de nulos. |
| `native_type` | string | Tipo base saneado opcional del motor, como `varchar` o `longtext`; sin identificadores, miembros de enumeraciones, valores predeterminados ni expresiones. Lo emiten los recopiladores nativos de MySQL y SQL Server. |
| `declared_max_chars` | int | Capacidad declarada opcional en caracteres. Exacta para los valores de catálogo `character`/`character varying` de PostgreSQL y en los modos MySQL equilibrado predeterminado y exacto; solo se redondea de forma aproximada con `--length-fidelity strict` de MySQL. |
| `declared_max_bytes` | int | Capacidad declarada opcional en bytes. Exacta en los modos MySQL equilibrado predeterminado y exacto; solo se redondea de forma aproximada con `--length-fidelity strict`. |
| `length_semantics` | cadena | V7: Unidad de longitud declarada opcional: `characters`, `bytes`, `not-applicable` o `unknown`. Esto preserva la semántica de CHAR frente a BYTE de Oracle sin serializar las declaraciones. |
| `numeric_model` | cadena | La versión V7 requiere un conjunto cerrado de familias: `integer`, `fixed-decimal`, `unconstrained-decimal`, `decimal-float`, `binary-float`, `not-applicable` o `unknown`. `not-applicable` indica un tipo no numérico conocido; `unknown` está reservado para un tipo numérico o definido por el usuario cuyas características no se han clasificado. `decimal-float` incluye valores exactos de Oracle `FLOAT(p)` y no es un número de punto flotante IEEE. |
| `numeric_precision` | int | Precisión declarada positiva opcional, limitada por el motor y modelo de origen: Oracle `NUMBER` y precisión decimal de SQL Server hasta 38, Oracle `FLOAT(p)` hasta 126 dígitos binarios, precisión decimal de MySQL hasta 65, y precisión numérica de PostgreSQL hasta 1000. |
| `numeric_scale` | int | Escala declarada con signo opcional, validada contra el motor de origen. Oracle `NUMBER` utiliza `-84..127`; PostgreSQL admite su rango de declaración más amplio y dependiente de la versión, mientras que MySQL, SQL Server, Parquet y Avro requieren una escala no negativa que no sea mayor que la precisión. Se conservan las escalas negativas Oracle/PostgreSQL y las escalas mayores que la precisión, siempre que el motor lo permita. |
| `numeric_precision_radix` | cadena | `decimal` o `binary` cuando lo requiera el modelo numérico. Oracle `FLOAT(p)` utiliza precisión binaria con el modelo de valor exacto `decimal-float`; `BINARY_FLOAT` y `BINARY_DOUBLE` utilizan `binary-float`. |
| `numeric_unsigned`, `bit_width` | bool / int | Semántica de enteros opcional, cuando el motor de origen los expone. |
| `datetime_precision` | int | Precisión fraccionaria opcional declarada por el motor date/time. |
| `charset`, `collation` | string | Metadatos opcionales saneados de caracteres. MySQL emite los nombres de catálogo de su juego de caracteres y su intercalación. SQL Server emite `utf-16le` para `nchar`/`nvarchar`/`ntext`, `utf-8` para la página de códigos 65001, `windows-N` para las páginas de códigos de Windows 1250–1258 o `code-page-N` para cualquier otra página de códigos positiva del catálogo, además del nombre de intercalación del catálogo. Son hechos de codificación y nombres de catálogo, nunca sus identificadores ni sus valores. |
| `len_avg` | int | Promedio muestreado de bytes para valores de longitud variable. Los intervalos relativos predeterminados tienen un error máximo de aproximadamente el 3,2 % y conservan exactamente los valores de hasta 32 bytes; es exacto con `--length-fidelity exact --yes`; el redondeo aproximado a la decena más cercana solo se utiliza en modo estricto. 0 = longitud fija o sin medir. |
| `len_p95` | int | Percentil 95 muestreado con los mismos intervalos relativos predeterminados; exacto con `--length-fidelity exact --yes`; el redondeo aproximado a la centena más cercana solo se utiliza en modo estricto. 0 = sin medir. |
| `style` | string | Solo nivel 2. Uno de `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` o `"precompressed"`; vacío si no se clasifica. `"precompressed"` solo se emite para una muestra de valores binarios materialmente dominante en bytes que contenga firmas reconocidas de contenedores estándar. No revela deliberadamente la familia de contenedor detectada. |
| `[tables.<id>.cols.<cid>.lob_storage]` | subtabla | Evidencia opcional de almacenamiento LOB de base de datos en la versión 7: `storage_class` (`basicfile`, `securefile`, `external`, `unknown`), compresión (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`), desduplicación (`enabled`, `disabled`, `not-applicable`, `unknown`), opciones in-row/encrypted, y visibilidad (`full`, `partial`, `unknown`). El contenido externo requiere que los dos controles de almacenamiento estén `not-applicable` y omite las opciones dentro de la base de datos. No se conserva ningún nombre de ruta o segmento. |
| `magnitude_min`, `magnitude_max` | int | Exponentes decimales con signo opcionales del esquema v6 que delimitan la magnitud de los números no NULL muestreados. Se emiten con `has_negative`; nunca se serializan valores exactos. |
| `has_negative` | bool | Observación opcional del signo en el esquema v6, emitida solo con ambos límites de magnitud. |
| `time_span` | string | Intervalo opcional de fecha/hora muestreado en el esquema v6: `intraday`, `days`, `weeks`, `months`, `years` o `decades`. |
| `time_recent_decade` | int | Década de la fecha/hora muestreada más reciente en el esquema v6, emitida solo con `time_span` y siempre divisible por 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Solo nivel 2. Presente para columnas candidatas de texto o binarias que se hayan muestreado. Misma disposición de campos que la compresión por tabla, pero limitada a una columna anonimizada. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Resumen de la distribución de valores muestreados del esquema v3. Solo contiene recuentos y frecuencias acotados o redondeados. |

`numeric_model` es la referencia autorizada para la semántica numérica. `type` mantiene la ortografía de la familia de motores: la familia `NUMBER` de Oracle se acompaña de `type = "number"` y `FLOAT(p)` por `"float"`, mientras que `native_type` conserva la declaración original, depurada.

### `[tables.<id>.cols.<cid>.cardinality]` (esquema v3)

Cuando el muestreo de filas está habilitado, el recolector mantiene en la memoria como máximo 8192 huellas digitales temporales de 64 bits por columna, calcula estadísticas agregadas NDV/skew y descarta las huellas digitales. Ni los valores ni las huellas digitales se serializan. El bloque contiene `measured`, `sample_rows`, `non_null_rows`, `observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`, `frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`, `sample_method`, `complete_source_read`, `sample_layout`, `sampled_with_bias` y `bias_reason`. `complete_source_read = true` es una evidencia legible por máquina que indica que una declaración limitada observó la población completa de la fuente y conservó esta columna sin truncar los valores. Una lectura completa de fila comprobada mantiene `sample_rows` dentro del dominio exacto de la fila de la tabla, incluso cuando un límite de celda o un reservorio de huellas digitales limitado hacen que `complete_source_read` sea falso; la población exacta de la tabla ya está presente en `tables.<id>.rows`, por lo que esto no revela ningún dato adicional. `non_null_rows` se calcula primero con fines de privacidad, y `null_fraction` se deriva posteriormente de `(sample_rows - non_null_rows) / sample_rows`. La fracción, por lo tanto, se mantiene exactamente consistente con los datos públicos y puede eliminarse de la cuadrícula independiente de 0.005 sin revelar otro dato. Los extremos de valor cero exacto y todos los valores que no son NULL se conservan; Un censo mixto mantiene una población positiva y no nula, y se mantiene por debajo de `sample_rows`. El `non_null_rows` mixto utiliza la misma cuadrícula de conteo relativo a la magnitud que los demás conteos de cardinalidad. En la parte final con mayor densidad, esto puede situar el conteo hasta un "bucket" completo por debajo de la población real retenida (por ejemplo, `9,728` para `9,999`); no es un recuento casi exacto. El punto final exacto, que no contiene valores NULL, revela deliberadamente que no se observó ningún valor NULL en las filas retenidas, mientras que cualquier valor NULL observado mantiene el conteo por debajo de `sample_rows`. Los conteos distintos y de frecuencia se mantienen dentro de su cuadrícula de privacidad documentada, incluso cuando están limitados por esa población. Para las fuentes de bases de datos, una lectura incompleta limita `sample_rows` al número estimado de filas del catálogo, redondeado. Para Parquet y Avro, el número exacto de filas en el pie de página es el límite. En ambos caminos, `sample_rows` es el número exacto de filas retenidas, ya revelado por el bloque de compresión a nivel de tabla, `sample_rows`, a menos que ese límite sea inferior. Nunca debe limitarse de forma que se sugiera una cobertura completa. La truncación de valores mantiene la evidencia de distinción y frecuencia de manera conservadora y nunca debe inflarla más allá de la población de la tabla. No infiera la integridad analizando `sample_method`. `sample_layout` es un enum opcional legible por máquina. El valor actual emitido es `primary-key-range-windows`; "ausencia" significa que no hay un contrato de ordenación disponible. No infiera el significado analizando el campo `sample_method` legible para humanos.

Las cantidades y las fracciones se redondean para proteger la privacidad, cuando es apropiado. Las estadísticas describen la densidad de duplicados, la distribución de valores frecuentes y los dominios finitos. No contienen valores muestreados, pero distribuciones distintivas pueden identificar una carga de trabajo; no las trate como irreversibles ni como prueba de que el significado empresarial no se puede inferir a partir de conocimientos externos. Una declaración limitada puede demostrar la población de filas visibles sin probar que cada celda muestreada se haya conservado por completo. Si un límite de celda del lado del servidor trunca una columna, su cardinalidad sigue siendo una estimación limitada y sesgada, incluso cuando el recuento de filas de la tabla se registra a partir de una lectura completa y limitada; las columnas no afectadas aún pueden tener la procedencia de cardinalidad de lectura completa.

### `[tables.<id>.cols.<cid>.compression]` (solo nivel 2)

La compresión por columna se genera solo para los candidatos text/binary con límites definidos cuando se utiliza `--measure-compression --yes`. Proporciona una estimación de la compresión por columna.

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
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

## `[tables.<id>.idxs.<iid>]`

El identificador es `idx-N`, donde `N` es el índice ordinal de 1 en el índice dentro de la tabla, ordenado por un HMAC-SHA256 separado por dominios del nombre del índice. El esquema v7 requiere el conjunto denso `idx-1` a `idx-N` para cada tabla; los ceros, los ceros iniciales, los espacios y los sufijos no decimales no son válidos.

| Campo | Tipo | Valores |
|---|---|---|
| `type` | string | Familia normalizada del método de índice, como `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` u `"other"`. No se emiten nombres de métodos personalizados o de extensiones. |
| `primary` | bool | Opcional; emitido como `true` para los índices de clave primaria. Se omite/falsa de lo contrario. |
| `unique` | bool |  |
| `cols` | array of int | ordinales de las columnas participantes, en el orden de las columnas del índice |
| `prefix_lengths` | array of int | Longitudes opcionales de prefijos de índices MySQL alineadas con `cols`; cero significa la columna completa. Exactas de forma predeterminada; solo se redondean hacia abajo con `--length-fidelity strict`. |
| `include_cols` | array of int | Opcional; ordinales de columnas INCLUDE que no forman parte de la clave cuando el motor de origen los expone. |
| `expression` | bool | Opcional; true cuando existe material de clave de expresión o función que no puede representarse como simples ordinales de columna. |
| `filtered` | bool | Opcional; true para índices filtrados o parciales. |
| `descending` | bool | Opcional; true cuando alguna columna de clave está explícitamente en orden descendente. |
| `partitioning` | cadena | V7: partición física opcional: `none`, `local`, `global` o `unknown`. |
| `visibility` | cadena | V7: Visibilidad opcional de la fuente: `visible`, `invisible` o `unknown`. |
| `state` | cadena | Estado operativo opcional V7: `usable`, `unusable`, `in-progress`, `failed` o `unknown`. |
| `prefix_distinct_counts` | array of int | Recuento estimado por el esquema v3 de tuplas distintas para cada prefijo de clave, desde una hasta N columnas. Cero significa que no está disponible para ese prefijo. |
| `cardinality_sample_method` | string | Procedencia acotada de `prefix_distinct_counts`; los productos inferidos se etiquetan explícitamente y no se presentan como muestras directas de tuplas. |

## `[tables.<id>.compression]` y `[tables.<id>.cols.<cid>.compression]` (solo nivel 2)

Se muestra solo cuando el archivo se generó con `--measure-compression --yes`. El bloque a nivel de tabla mide una proyección columnar neutral del conjunto de muestras completo y sigue siendo la proporción autorizada para las estimaciones de transferencia de tablas completas. Los bloques a nivel de columna se proyectan a partir de las mismas filas muestreadas, una columna a la vez, y muestran qué columnas se comprimen bien sin exponer los valores muestreados. No desencadenan lecturas adicionales de la base de datos.

Las tablas de PostgreSQL que están sujetas a seguridad a nivel de fila, incluyendo las tablas hijas heredadas o particionadas cuya política ancestral sería omitida por una consulta de un hijo directo, y las tablas de SQL Server que están sujetas a un predicado de filtro de seguridad habilitado, no se muestrean. Se conserva la evidencia del catálogo, y los registros de ejecución `DBP1407W`, en lugar de extrapolar un subconjunto filtrado por la política como si fuera toda la tabla.

| Campo | Tipo | Precisión |
|---|---|---|
| `measured` | bool | siempre `true` si el bloque está presente |
| `sample_rows` | int | exacta |
| `sample_bytes` | int | tamaño del búfer de muestras en memoria, **agrupado por intervalos**: al múltiplo de **64 KiB** más cercano por debajo de 1 MiB, al **1 MiB** más cercano por debajo de 1 GiB y al **100 MiB** más cercano por encima. Los bytes nunca se escriben en disco. La agrupación elimina el canal encubierto de bits bajos por tabla que expondría un valor exacto de `buf.len()`. |
| `sample_method` | string | descripción del muestreo acotado específica del motor, por ejemplo `"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`, `"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"` o `"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"` |
| `sampled_with_bias` | bool | true si la muestra no es uniforme, por ejemplo un mecanismo alternativo que solo utilice LIMIT |
| `bias_reason` | string | Cuando `sampled_with_bias = false`, este campo está vacío. De lo contrario, contiene una etiqueta como `"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"`. |
| `ratio_zstd_3` | float | redondeada al múltiplo de **0.05** más cercano, según la política de medición zstd de nivel 3 del contrato. Medida sobre bytes codificados mediante `sample_encoding`. |
| `ratio_zstd_19` | flotante | No escrito en esta versión; puede aparecer en archivos de versiones anteriores. |
| `ratio_stddev` | float | redondeada al múltiplo de **0.05** más cercano, desviación estándar de las proporciones de nivel 3 sobre tramas acotadas de la sonda de tabla. Los bloques de proyección por columna emiten actualmente `0.0` porque son indicios orientativos de entropía, no un modelo de varianza. |
| `sample_encoding` | cadena | Identificador para la política de codificación y compresión a nivel de bytes utilizada para la medición. Los bloques de tablas en vivo de PostgreSQL utilizan `"blueprint-columnar-transfer-probe-v2"`. MySQL y SQL Server utilizan `"blueprint-columnar-transfer-probe-v3"`, que además realiza un vaciado a los límites de bloques de 256 KiB. Las cargas útiles de SQL Server `nvarchar`/`nchar`/`ntext` conservan la distribución de bytes nativa UTF-16LE; `varchar`/`char`/`text` conservan su ancho de byte muestreado, y el campo `charset` identifica la página de códigos del catálogo. V1 se acepta como entrada. Los bloques por columna utilizan `"blueprint-compression-probe-v2"`. Las relaciones medidas con diferentes valores de `sample_encoding` no son comparables. |

PostgreSQL utiliza la versión 2, mientras que MySQL y SQL Server utilizan la versión 3; compare las proporciones solo dentro de una misma codificación.

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

`ratio_zstd_3` describe el `sample_encoding` nombrado; no es una captura de bytes del protocolo de la base de datos ni de la transmisión de la migración. El conjunto de pruebas en este repositorio valida la codificación determinista, el muestreo limitado y la serialización, pero no afirma un porcentaje de error universal entre diferentes motores para cada ruta de extracción.

Antes de utilizar la relación para una decisión importante sobre la capacidad, verifique la relación con datos de origen representativos y el mecanismo de extracción previsto. Registre el método de comparación, el tamaño de la muestra, el hash binario, la versión del motor y el error observado junto con el plan resultante. La relación primitiva es `compressed_bytes ≈ sample_bytes / ratio_zstd_3` bajo la distribución de bytes producida por la codificación registrada.

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
inferidas.

## `[artifact_inventory]` (desde la versión del esquema v4; obligatorio en la versión v7)

La versión del esquema v7 utiliza el contrato `dbwarp-blueprint-artifacts/v2`, que se versiona de forma independiente, para describir objetos que no son tablas, sin serializar los nombres ni las definiciones de la fuente. Las versiones anteriores del esquema conservan el contrato v1. La versión v7 siempre emite este bloque: `--artifact-detail none` registra explícitamente un inventario de la base de datos que no se solicitó, mientras que las fuentes de archivos estructurados emiten un inventario explícito que no es aplicable. Por lo tanto, un bloque faltante nunca se confundirá con un catálogo vacío verificado.

El valor predeterminado `--artifact-detail summary` emite `object_count`,
`external_prerequisite_count`, `counts_by_kind` y
`counts_by_external_class`. `graph` añade un registro de objeto anónimo por
artefacto y aristas de dependencia. `analyzed` añade registros acotados de
`dbwarp-language-feature-census/v1` derivados temporalmente de las definiciones
disponibles. `graph` y `analyzed` requieren `--yes` explícito porque la
topología del grafo puede identificar una aplicación.

`object_count` es el número de registros de artefactos emitidos por el recolector, no el número de filas devueltas por ningún catálogo nativo. Por lo tanto, un paquete o tipo puede contribuir con registros de especificación, cuerpo y miembros separados. Un objeto nativo que aparece en más de un catálogo sigue siendo un único registro: por ejemplo, las filas de disparador de Oracle de los catálogos de disparador y origen se unen mediante su identidad de objeto nativo, y las filas de origen enriquecen en lugar de duplicar el registro del disparador.

Los paquetes y los tipos de objeto de Oracle utilizan la misma estructura de registro: un registro `specification`, un registro `body` vinculado como su implementación, y un registro `package_member` procedure/function por cada miembro del catálogo, con la especificación como elemento padre. Solo el cuerpo posee el texto fuente combinado y el recuento de lenguaje; los miembros conservan sus datos del catálogo, pero utilizan un análisis de definición no aplicable. Esto evita que un analizador léxico pretenda que puede dividir el código fuente del paquete en los cuerpos de los miembros.

La evidencia del inventario incluye:

| Campo | Valores / regla |
|---|---|
| `detail` | `none`, `summary`, `graph` o `analyzed` |
| `scope` | V7: `all-visible-schemas`, `selected-schemas`, `structured-source` o `unknown`; debe coincidir con la evidencia de selección de esquema en otra parte del archivo. |
| `visibility` | `full`, `privilege_filtered` o `unknown` |
| `inventory_complete` | Solo puede ser verdadero con visibilidad completa, sin catálogos ilegibles ni familias sin modelar declaradas |
| `dependencies_complete` | Solo puede ser verdadero si se pudieron leer los catálogos de dependencias modelados |
| `requirements_complete` | Agregado V7: verdadero solo con cobertura completa de la población de evaluación para el alcance seleccionado y `requirement_status = complete | not_applicable` en cada artefacto emitido; la omisión significa falso y una lista de requisitos vacía no demuestra completitud |
| `analysis_complete` | Solo puede ser verdadero con detalle analyzed y si todos los análisis emitidos están completos |
| `catalogs_read` | Etiquetas cerradas y estándar de catálogos de motor inspeccionados correctamente |
| `catalogs_unreadable` | Las etiquetas del catálogo que fallaron; cada entrada impide que se cumplan las afirmaciones de integridad proporcionadas por ese catálogo, mientras que la evidencia de requisitos específica de cada objeto puede permanecer completa. |
| `catalogs_not_applicable` | Las etiquetas del catálogo V7 resultaron no ser aplicables; estaban separadas de los catálogos legibles e ilegibles. |
| `families_not_inventoried` | Familias de objetos conocidas que no están inventariadas en esta versión |

### `[artifact_inventory.complexity]` (esquema v7)

El bloque `dbwarp-blueprint-artifact-complexity/v1` es una evaluación que solo considera agregados, basada en el censo de artefactos anónimos. Está ausente en los detalles de `none` y `summary`, y es requerido en los detalles de `graph` y `analyzed`, donde su presencia significa que se intentó realizar la evaluación. Un fallo en el cálculo produce un resultado desconocido que indica un fallo, en lugar de abortar el Blueprint.

Los campos de nivel superior son fijos:

| Campo. | Valores / regla. |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | Debe ser exactamente igual a `artifact_inventory.scope`. |
| `population_policy` | `exclude-known-engine-generated-and-secondary`; los indicadores ausentes siguen siendo elegibles y los objetos temporales siguen siendo elegibles. |
| `assessment_population_complete` | Solo es cierto cuando se conoce cada objeto que cumple con la política de población; la omisión significa falso, y esta afirmación es independiente del campo `inventory_complete` más amplio. |
| `eligible_object_count` | Objetos evaluados por la política. |
| `fully_assessed_object_count` | Cada dimensión es conocida o está probada `not-applicable`. |
| `partially_assessed_object_count` | Al menos una dimensión aplicable conocida y al menos una desconocida. |
| `unassessed_object_count` | No se conoce ninguna dimensión aplicable. |
| `excluded_object_count` | Objetos excluidos por la política registrada. |
| `analyzer_version` | El único analizador utilizado por la captura de la versión 7: `lexical-v2`, o `not-applicable` en modo de gráfico. |
| `analysis_spans` | Tokens de ámbito únicos, ordenados y cerrados presentes en los registros censales elegibles: `executable-body`, `not-applicable` o `unknown`; vacíos en modo gráfico. |
| `dialects` | Tokens de dialecto cerrados, únicos y ordenados, presentes en los registros censales elegibles. |
| `grammar_profiles` | Perfiles gramaticales únicos y ordenados presentes en los registros censales elegibles. |
| `overall_band` | `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable`, o `unknown` |
| `overall_score` | No escrito por esta versión. |
| `limitations` | Las razones de cierre ordenadas se describen a continuación. |

Ambas ecuaciones de población utilizan aritmética verificada:

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions` contiene exactamente `volume`, `control_flow`, `feature_breadth`, `entanglement`, `environment_coupling`, `opacity` y `dialect_coupling`. Cada dimensión tiene una `band` cerrada, un valor `coverage` (`complete`, `partial`, `not-applicable` o `unknown`), y un histograma fijo. La `band` de la dimensión `volume`, al igual que las otras dimensiones, utiliza `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable` o `unknown`. Su histograma utiliza las claves de tamaño `0`, `1-255`, `256-1k`, `1k-4k`, `4k-16k`, `16k-64k` y `64k+`. Los otros seis histogramas utilizan las claves de conteo `0`, `1`, `2-4`, `5-8`, `9-16`, `17-32` y `33+`. Cada histograma también tiene cubetas `not_applicable` y `unknown`. Para cada dimensión, la comprobación aritmética requiere:

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

La cobertura, por lo tanto, es por dimensión, no un único indicador a nivel de entidad. Un resultado de censo `not_applicable` es una evidencia determinada y contribuye al "bucket" `not_applicable` de la dimensión; no significa que el objeto no se haya evaluado. Los conteos del objeto fully/partially/unassessed de nivel superior son un resumen derivado: todos los objetos que no son aplicables están completamente evaluados, "parcial" significa que al menos una dimensión aplicable es conocida y otra es desconocida, y "no evaluado" significa que no se conoce ninguna dimensión aplicable.

Una dimensión parcial se evalúa como un límite inferior y superior. Su `band` es `unknown` a menos que el límite inferior conocido ya sea `very-high`, porque cualquier observación desconocida puede ocupar el rango más alto. Esto evita que un histograma parcial presente su límite inferior observado como un veredicto final.

`external_binary` es un estado de visibilidad de la definición, no una bandera de exclusión. Los complementos instalados en el sitio, los ensamblajes CLR, los objetos Java y las bibliotecas externas siguen siendo elegibles para la migración y normalmente contribuyen con evidencia dependiente de la definición, desconocida. Solo la bandera `generated_by_engine = true` explícita excluye un objeto proporcionado por el motor, según el evaluador v1.

Los histogramas son deliberadamente unidimensionales. Las tablas cruzadas por tipo, característica, esquema o cualquier otro atributo no forman parte del contrato. Los recuentos exactos no aportan información adicional más allá del censo serializado por objeto en modo analizado, mientras que la forma fija evita publicar una herramienta oficial para identificar la estructura de datos.

Las razones de limitación cerradas son `definition-analysis-not-requested`, `definitions-withheld`, `unsupported-dialect`, `wrapped-source`, `graph-incomplete`, `requirements-incomplete`, `outside-selected-scope`, `computation-limit` y `computation-failed`. `requirements-incomplete` significa que la captura de requisitos no está completa. Los objetos con estado de requisito `partial` o `unavailable` contribuyen con observaciones desconocidas en lugar de cero en cuanto al acoplamiento del entorno y el dialecto; los objetos completos e independientes permanecen evaluados. `unsupported-dialect` significa que la definición se obtuvo, pero su idioma o dialecto no tiene un analizador compatible; no se retiene ni se vuelve deliberadamente opaca. El detalle del gráfico utiliza `definition-analysis-not-requested`; no debe afirmar una limitación de lectura de definición porque no se intentó ninguna lectura de este tipo.

La evidencia desconocida se limita de forma independiente para cada dimensión afectada. La banda general se emite solo cuando las evaluaciones inferior y superior coinciden. Una población completa y vacía es `not-applicable`, nunca `trivial`. El modo de gráfico siempre utiliza una `unknown` general para una población no vacía, porque no lee las definiciones requeridas por la evaluación general. La falta de aristas en el gráfico hace que la evidencia de entrelazamiento afectada sea desconocida. `computation-limit` no se escribe en esta versión. Un fallo inesperado en el cálculo registra `computation-failed`, conserva el inventario completo de artefactos y marca la evaluación agregada como "fallo cerrado" en lugar de suprimirla.

`assessment_population_complete`, en lugar de `artifact_inventory.inventory_complete` de forma general, determina si el resultado limitado puede ser definitivo. Si la población evaluada está incompleta, el rango general es `unknown` a menos que el límite inferior conocido ya sea `very-high`; no se asume un límite superior finito para los objetos que pueden ser invisibles.

Los objetos completamente incluidos contribuyen `unknown` a la opacidad; no se omiten del histograma de opacidad simplemente porque no se pudo producir un censo parcial. Presente la opacidad junto con su cobertura para que una banda de región opaca observada baja no pueda ocultar una gran población desconocida.

`unsupported-dialect` sigue siendo una limitación distinta porque el censo puede identificar un dialecto e informar `unavailable`, pero no tiene un estado `unsupported`. Se deriva únicamente cuando una definición estaba disponible y el dialecto registrado no es compatible con el analizador especificado. Otras limitaciones de la definición se derivan igualmente de la visibilidad de la definición, el estado del censo y la evidencia del artefacto, en lugar de mantenerse como una afirmación independiente.

La elegibilidad para la comparación se calcula entre archivos provenientes del contrato de complejidad, la versión del evaluador, la versión del analizador, el intervalo de análisis exacto, los conjuntos de dialectos y perfiles gramaticales, el alcance y la política de población. Una opción homogeneous/mixed de granularidad gruesa no se serializa porque diferentes conjuntos mixtos no son necesariamente comparables.

La complejidad siempre es específica de la fuente. Un paquete conserva la evaluación de cada Blueprint hijo y nunca crea una banda de complejidad a nivel de paquete ni un histograma a través de los motores, las versiones del analizador, los dialectos o los perfiles de gramática.

Los identificadores de cada objeto tienen el formato `<kind>-NNN`, como `view-001`, `package-002` o `procedure-003`. V7 reconoce las familias de objetos comunes, además de los paquetes de Oracle, los objetos de programación, los enlaces de base de datos, los directorios, las bibliotecas, los objetos Java, los operadores, los tipos de índice, los dominios, las anotaciones y los grafos de propiedades, así como los tipos `queue` y `edition` independientes del motor. El mínimo de tres dígitos se rellena con ceros y cada tipo tiene su propio conjunto ordinal denso que comienza en `001`; el ancho aumenta por encima de 999. El registro contiene solo tokens kind/subkind/tier cerrados, identificadores schema/parent anónimos, modo de definición visibility/security, indicadores de catálogo y validez opcionales, cobertura de requisitos cerrados, un prerrequisito externo opcional y un censo de lenguaje opcional. Un elemento padre puede ser una tabla anónima u otro artefacto, por lo que se puede preservar una jerarquía de paquetes a procedimientos sin nombres; los grafos de elementos padre deben ser acíclicos.

V7 utiliza un único vocabulario cerrado `subkind` en todos los motores:

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

V7 reemplaza la lista de dependencias ambigua v1 con una lista tipificada y ordenada `relationships`. Los tipos de relación distinguen entre llamadas, lecturas, escrituras, referencias table/object, propiedad de disparadores, implementación, ubicación física, seguridad, uso de extensiones, referencias externas binaries/services y uso remoto database/server. Cada relación registra un token cerrado de evidencia (`catalog-confirmed`, `dependency-confirmed`, `syntax-confirmed`, `lexical-hint` o `unresolved`). `dependency_edge_count` debe ser exactamente igual al grafo emitido.

`requirements` Utilice tokens calificados por el motor y un rango de conteo limitado. Estos identifican necesidades de compatibilidad, como una fuente "wrapped" de Oracle, un disparador compuesto, el estado del paquete, SQL dinámico, una transacción autónoma, pipelined/parallel/aggregate rutina, biblioteca externa, enlace de base de datos, índice de dominio, programador, object/collection/spatial/vector tipo, objeto Java o gráfico de propiedades. Son solo evidencia de planificación.

Los requisitos provienen de un catálogo delimitado o de una verificación de sintaxis específica para el motor. El análisis léxico genérico nunca genera un requisito específico para el motor. Cada grafo de esquema v7 o artefacto analizado lleva `requirement_status = complete | partial | unavailable | not_applicable`. `complete` establece que la lista es exhaustiva para ese artefacto; `not_applicable` establece que el modelo de requisitos no se aplica y, por lo tanto, prohíbe los registros de requisitos y prerrequisitos externos. `partial` y `unavailable` hacen que las observaciones de compatibilidad con el entorno y el dialecto de ese objeto sean desconocidas. `partial` significa que al menos una fuente de datos tuvo éxito sin una cobertura exhaustiva; `unavailable` significa que ninguna fuente de requisitos estableció una cobertura utilizable y, por lo tanto, no puede contener evidencia conocida de requisitos o prerrequisitos externos. Tal evidencia requiere `partial`. Esto permite que un objeto inaccesible se degrade localmente en lugar de borrar la cobertura útil para el resto del sistema.

El nivel de inventario `requirements_complete` es la afirmación agregada. Solo puede ser verdadero cuando cada artefacto generado es `complete` o `not_applicable` y la población de evaluación está completa para el alcance seleccionado; puede permanecer falso incluso cuando cada artefacto es `complete`. Nunca interprete un array `requirements` vacío como cero acoplamiento a menos que el estado de ese artefacto sea `complete`.

`unresolved_relationships` es un mapa delimitado que relaciona motivos y conteos. Distingue referencias remotas y entre bases de datos, límites de esquema seleccionados, objetivos con privilegios ocultos, definiciones encriptadas o retenidas, SQL dinámico, enlaces ambiguos, identidades nativas faltantes o incompletas, familias de objetivos no modeladas y casos desconocidos. La evidencia completa de dependencias requiere que este mapa esté vacío. Los nombres de objetos de origen, el texto SQL, los principales de seguridad, los puntos finales, las credenciales, las claves, los certificados y los binarios no son campos del contrato.

Los requisitos externos registran una `class` cerrada, el ámbito de despliegue,
si se requiere material binario/secreto/de punto de conexión no capturado y una
categoría de compatibilidad acotada. Su recuento es evidencia de planificación
de la migración, no una afirmación de que DBWarp pueda aprovisionarlos o
traducirlos automáticamente.

Los registros de conteo de lenguaje V7 utilizan `analyzer_version = "lexical-v2"` y registran `analysis_span`. El analizador solo recibe el cuerpo ejecutable o declarativo, excluyendo el envoltorio de creación externo, la identidad, la firma, la declaración de retorno y las opciones del módulo. Los hechos del encabezado permanecen como requisitos del catálogo o opciones. Un recolector que no puede aislar de forma segura el cuerpo registra `analysis_span = "unknown"` y evidencia no disponible en lugar de analizar el envoltorio. Una definición no aplicable comprobada utiliza `analysis_span = "not-applicable"`. La evidencia de un rango omitido se interpreta de forma conservadora como `unknown`; nunca se infiere del motor ni del tipo de objeto. Una definición compatible analizada por esta implementación léxica utiliza `status = "partial"`; la evidencia de una definición faltante o no compatible utiliza `unavailable`, y un objeto no aplicable comprobado puede utilizar `not_applicable`. El conteo, el tamaño, el anidamiento, la complejidad y los valores de la región opaca son rangos, no huellas digitales exactas del origen. Las características se seleccionan de un vocabulario cerrado. El analizador elimina comentarios, literales e identificadores entre comillas; no es un analizador, un vinculador semántico ni una garantía de éxito de la traducción.

"Wrapped PL/SQL nunca es evidencia del cuerpo ejecutable. El recolector lo marca como encriptado y retiene sus bytes del análisis; el analizador compartido también rechaza una unidad PL/SQL cuyo encabezado contiene el marcador "wrapped", evitando que un error de clasificación produzca rangos de población plausibles pero falsos."

Consulte el [Inventario de artefactos no tabulares](ARTIFACT_INVENTORY.md) para
la guía operativa y la cobertura de motores.

## Defensas contra la esteganografía, por vector

| Vector | Defensa |
|---|---|
| Ordenamiento de identificadores. | HMAC-SHA256 con separación de dominio y una clave local al proceso evita las comprobaciones de nombres de candidatos fuera de línea. Reutilice una clave solo cuando se requieran etiquetas estables entre ejecuciones. |
| Bits bajos numéricos | Las estadísticas se redondean de forma predeterminada con la precisión documentada. El modo de longitudes exactas es explícito, requiere consentimiento, se registra en el registro de auditoría y debe tratarse como metadatos más sensibles. |
| Marca de tiempo inferior a un segundo | Una marca de tiempo UTC en la cabecera, solo con resolución de segundos |
| Formato TOML | La salida estándar utiliza un orden de claves y una sangría que no varían. Incluye solo el encabezado estándar y los comentarios del productor, sin comentarios derivados de la entrada. |
| Aleatoriedad en el muestreo. | El muestreo utiliza semillas fijas (deterministas de PG `TABLESAMPLE SYSTEM`). Por separado, la anonimización de identificadores obtiene intencionalmente una clave secreta del generador de números aleatorios criptográficamente seguros (CSPRNG) del sistema operativo, a menos que usted proporcione una. |
| Campos sin utilizar | Todos los campos se documentan arriba; no hay campos "metadata"/"comment"/"reserved" que contengan datos sin límites |
| Texto fuente de artefactos y material externo | Las definiciones son transitorias y se borran tras el análisis acotado; nombres, texto SQL, puntos de conexión, cadenas de proveedor, credenciales, claves, certificados, nombres de paquetes y binarios no tienen ningún campo serializado |

## Compatibilidad de versiones del esquema

Los productores actuales emiten la versión 7 del esquema. Las versiones 1 a 6 siguen siendo aceptadas para la compatibilidad con versiones anteriores. Un archivo v1/v2 no tiene bloques de distribución. Un archivo de versión 3 tiene metadatos de distribución, pero no tiene un inventario de artefactos. Un archivo de versión 4 puede contener un inventario de artefactos, pero es anterior a los identificadores actuales del contrato Blueprint. Los lectores normalizan los antiguos identificadores de la versión 4 en la entrada y reemiten ese documento con los identificadores canónicos de Blueprint. Un archivo de versión 5 es anterior a la evidencia de topología y alcance del conjunto de datos que se agregó en la versión 6. La versión 6 utiliza el contrato de topología v1, el contrato de artefactos v1, los campos combinados de la tabla kind/partition, la escala decimal sin signo y las estadísticas opcionales de frescura. La versión 7 utiliza el contrato de topología v2 y el contrato de artefactos v2, requiere evidencia explícita de structure/environment/statistics, separa la semántica ortogonal de la tabla, admite la escala decimal con signo y los modelos numéricos de Oracle, y requiere un estado explícito del inventario de artefactos. También reserva el contrato agregado fijo `dbwarp-blueprint-artifact-complexity/v1` sin agregar un puntaje por objeto o un campo de tabla cruzada. Los lectores rechazan las versiones de esquema futuras desconocidas con un mensaje de actualización claro en lugar de descartar silenciosamente los campos. Los lectores aplican la misma validación estricta de la versión 7 a los Blueprints independientes y integrados en lugar de reescribir la evidencia no válida durante el análisis.

## Por qué TOML y no JSON

- TOML separa de forma más legible las secciones estructurales de los datos de
  hoja (`[tables.table-001.cols.col-2]` frente a JSON anidado).
- Es más fácil de comparar (una clave por línea; las subtablas basadas en
  identificadores permanecen contiguas).
- Revise de acuerdo con la política de clasificación de datos de su organización antes de compartir.

JSON se utiliza como **formato intermedio** en la ruta SQL alternativa. Cada
script `sql/blueprint.*.sql` produce JSON y `blueprint_format.py` lo normaliza a
TOML. El JSON intermedio contiene identificadores reales del origen; MySQL
también puede incluir declaraciones enum/set mediante `COLUMN_TYPE`, por lo que
debe permanecer protegido dentro del entorno de origen. El normalizador usa una
clave secreta nueva de forma predeterminada y acepta el mismo contrato protegido
de `--anonymization-key-file` para comparaciones aprobadas entre ejecuciones. El
archivo final revisado para compartirlo con DBWarp siempre es TOML.

## Extensiones de procedencia para archivos estructurados

Cuando `engine` o `source_kind` es `"parquet"` o `"avro"`, la versión 3 o superior del esquema también puede emitir los siguientes campos delimitados. Los lectores deben preservar la distinción entre el almacenamiento de archivos de origen y las mediciones de muestra decodificadas delimitadas; los lectores que no admitan la versión del esquema del documento deben rechazarlo con un mensaje de actualización en lugar de descartar campos desconocidos.

Los Blueprints de archivos estructurados de V7 requieren bloques `[structure_scope]` y `[statistics_evidence]` completos, omiten `[database_topology]`, `[source_environment]` y `[activity_snapshot]`, y emiten un `[artifact_inventory]` explícito de "no aplicable". Nunca infieren la topología de la base de datos ni la capacidad del servidor-máquina a partir del host del recolector.

Los Blueprints de archivos estructurados usan los mismos identificadores
anonimizados que las Blueprints de bases de datos: `table-NNN` en orden basado en una clave secreta
y `col-N` en orden ordinal del esquema. Los nombres base de archivo,
las rutas Parquet, los nombres de campo Avro y el valor `logical_table` del
manifiesto no se emiten como identificadores de tabla o columna.

A nivel de tabla, `table_bytes` es la estimación lógica del tamaño de la transferencia, mientras que `storage_bytes` es el tamaño real del objeto de origen en disco. Parquet solo de metadatos utiliza bytes de fragmentos de columna sin comprimir para `table_bytes`; el muestreo decodificado opcional reemplaza esa estimación con los bytes proyectados `blueprint-compression-probe-v2`. Avro lo deriva de su escaneo completo decodificado. Los campos opcionales `source_partitions`, `row_group_count` y `source_codec` describen la estructura del archivo. Los conjuntos de datos de varios archivos agregan estos valores. `row_group_count` es específico de Parquet; `source_partitions` es `1` para un único objeto de entrada.

En una columna, `null_fraction` es un valor observado entre `0.0` y `1.0`.
`length_sample_rows` y `length_sample_method` indican cómo se obtuvieron
`len_avg` y `len_p95`. `source_semantics` conserva hechos acotados de
compatibilidad como `"repeated-leaf"`, `"nested-json"` o `"multi-type-union"`;
nunca contiene sus nombres de campo ni sus valores. La precisión y la
escala decimales, la precisión y semántica UTC/local de marcas de tiempo, UUID
y el tamaño binario fijo se conservan en los campos escalares saneados
existentes y `native_type`.

En una tabla, `ratio_storage` compara `table_bytes` con los bytes reales del
objeto de origen. En una columna Parquet compara los bytes sin comprimir y
comprimidos del fragmento de columna del footer. Ambos son señales para
planificar el almacenamiento de archivos, no estimaciones de muestras
decodificadas. `ratio_zstd_3` y `ratio_zstd_19` solo son comparables cuando
`sample_encoding` es `"blueprint-compression-probe-v2"`. Los ratios del footer
Parquet o del contenedor Avro nunca deben copiarse a esos campos zstd.
