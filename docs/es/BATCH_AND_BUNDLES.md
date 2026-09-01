# Recopilación por lotes y paquetes Blueprint

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../BATCH_AND_BUNDLES.md).

**Idiomas:** [English](../BATCH_AND_BUNDLES.md) | [Deutsch](../de/BATCH_AND_BUNDLES.md) | [Français](../fr/BATCH_AND_BUNDLES.md) | **Español** | [Polski](../pl/BATCH_AND_BUNDLES.md) | [日本語](../ja/BATCH_AND_BUNDLES.md) | [中文](../zh/BATCH_AND_BUNDLES.md)

`dbwarp-blueprint` admite tanto archivos Blueprint de un solo origen como
directorios de paquetes con varios orígenes.

Utilice un solo `blueprint.toml` cuando el cliente comparta una base de datos, un
subconjunto de tablas, un archivo Parquet o un archivo Avro. Utilice un paquete
cuando el cliente tenga varias bases de datos, varios conjuntos de datos de
archivos estructurados o desee un único paquete de revisión para todo su
entorno.

## Disposición del paquete

Una ejecución por lotes escribe un directorio:

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` contiene metadatos de cada origen y rutas relativas a los archivos
Blueprint secundarios. Esta es la forma de trabajo preferida porque cada origen
se puede revisar, auditar y volver a ejecutar de manera independiente.

Para una entrega revisada por separado, empaquete el directorio en un único
TOML integrado:

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

La forma empaquetada integra cada Blueprint secundario en su entrada de origen. Conserva los identificadores de origen, las etiquetas, los identificadores de grupo de conjuntos de datos y los metadatos de rutas de auditoría proporcionados por el operador; utilice valores anónimos en el manifiesto e inspeccione el archivo empaquetado antes de transferirlo. El directorio de trabajo resulta más fácil de revisar, pero también contiene auditorías detalladas y cualquier archivo `errors.txt`; no lo transfiera completo de forma predeterminada.

## Contrato del paquete

Los paquetes actuales usan `schema_version = 3` y
`kind = "dbwarp-blueprint-bundle"`. Un paquete en directorio referencia cada
Blueprint secundario mediante `blueprint_path`; un paquete integrado lo incluye
en `blueprint`. Los escritores solo emiten estos identificadores canónicos.

Los lectores también aceptan los esquemas de paquete v1 y v2. Esos contratos
solo ofrecen compatibilidad de entrada: un paquete heredado aceptado se
normaliza a v3 y nunca vuelve a emitirse con identificadores anteriores. Como
los paquetes antiguos no indican si las fuentes son independientes, réplicas o
fragmentos, su relación pasa a `unknown` y se suprimen los totales entre
fuentes. Las rutas secundarias deben ser relativas y permanecer dentro del
directorio tras la canonicalización.

El paquete v3 separa fuentes físicas de captura y conjuntos de datos lógicos.
Cada fuente tiene `dataset_relationship`, `dataset_group` y
`dataset_scope_completeness`. La tabla superior `dataset_groups` registra la
relación, los miembros y si el conjunto declarado está completo.

La agregación falla de forma segura:

- `independent`: exactamente una fuente en el grupo; sus totales se suman una
  vez.
- `replica`: las copias coincidentes cuentan una vez. Si divergen, se conserva
  un representante determinista, sin promediar, y el resultado es incompleto.
- `shard`: los miembros solo se suman cuando `members_complete = true` y todos
  los declarados han finalizado correctamente. Un grupo incompleto no aporta
  totales.
- `unknown`: se suprimen todos los totales de tablas, filas y bytes entre
  fuentes.
- Una fuente cuyo `[dataset_scope]` sea incompleto o desconocido hace que la
  evidencia agregada sea incompleta aunque su relación sea conocida.

Los totales por fuente siempre se conservan. La supresión solo afecta al
agregado entre fuentes, evitando multiplicar réplicas o presentar un conjunto
parcial de fragmentos como el conjunto completo.

## Manifiesto de lote

Cree un manifiesto propiedad del cliente:

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

Si se omite la relación, el valor predeterminado es `unknown`; la ejecución
termina, pero emite `DBP1414W` y `DBP1417W` y suprime los totales agregados. Es
más seguro que suponer que dos endpoints son dos conjuntos independientes.

Declare los miembros replicados con un grupo compartido:

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

En sistemas fragmentados, enumere cada fragmento conocido en un grupo común y
establezca `dataset_group_complete = true` solo si el manifiesto enumera el
conjunto lógico completo. Un miembro fallido hace que el grupo quede incompleto
en esa ejecución.

Realice primero una simulación:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Ejecute el lote:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Una ejecución por lotes que no sea una simulación requiere `--yes` porque puede
conectarse a varias bases de datos o decodificar muestras de archivos
estructurados. Cada origen secundario obtiene su propio archivo de auditoría.

Con `continue_on_error = true`, se procesan los orígenes restantes y se publica atómicamente el paquete de diagnóstico, incluido `errors.txt`. Aun así, el comando termina con error: `DBP1115E` si fallaron todos los orígenes y `DBP1116E` si el fallo fue parcial. Un paquete parcial sirve para revisión y reintento; no es una recopilación completa correcta.

Tanto la simulación como la ejecución real validan el manifiesto completo antes
de acceder a un origen. Se rechazan los campos desconocidos, los identificadores
duplicados, los identificadores que colisionan después de normalizar de forma
segura el nombre de archivo, los campos incompatibles con el tipo de origen, los
orígenes de conexión a bases de datos ambiguos, los modos de conjunto de datos
no válidos y los presupuestos de muestreo de compresión iguales a cero. Cada
`source.id` debe ser único, no contener espacios iniciales ni finales y tener
como máximo 120 bytes ASCII después de la normalización.

## Modos de conjuntos de datos de archivos estructurados

Para orígenes Parquet y Avro:

- `single_file` requiere exactamente un archivo resuelto y lo mantiene como una tabla lógica.
- `one_table_per_file` asigna cada archivo a una tabla saneada independiente en
  un archivo Blueprint secundario.
- `merge_same_schema` combina muchos archivos en una tabla lógica cuando
  coincide el número de columnas.
- `partitioned_dataset` utiliza actualmente el mismo comportamiento de
  combinación que `merge_same_schema`; reserva la distinción semántica para el
  descubrimiento de particiones al estilo Hive.

La comprobación de combinación es deliberadamente conservadora. Requiere que
coincidan la disposición anonimizada de columnas, los tipos canónicos y nativos,
la nulabilidad, los anchos declarados, la precisión y escala, la semántica sin
signo y de `BIT(n)`, la precisión de las marcas de tiempo, el juego de caracteres
y la intercalación, y la semántica del origen estructurado. Para la planificación
crítica de lagos de datos, mantenga agrupados los conjuntos de datos cuyo esquema
se conozca incluso cuando esta comprobación estructural sea satisfactoria.

## Operaciones con paquetes

Enumerar los orígenes:

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Las primeras líneas muestran `aggregation`, las `sources` físicas,
`logical_datasets`, los totales agregados y las `limitations`. Las líneas de
grupo muestran `relationship`, `members_complete` y los identificadores de
fuente. Las líneas de fuente muestran `dataset_relationship`, `dataset_group`
y `dataset_scope`. Interprete `aggregation=suppressed` como una instrucción para
revisar o corregir el manifiesto, no como un entorno de tamaño cero.

Enumerar un subconjunto de orígenes con una etiqueta determinada:

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

Extraer un origen:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Extraer una tabla de un origen:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Las claves de selector admitidas son:

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

Los selectores pueden pasarse como una cadena separada por comas o mediante
varias opciones `--select`. Se rechazan los valores contradictorios para una
misma clave.

## Entrega posterior

Un paquete es una entrada Blueprint portátil y revisable. Antes de aceptarlo, un consumidor posterior debe validar el contrato del paquete y las versiones de esquema, aplicar los selectores registrados y conservar los identificadores de origen al combinar varios elementos secundarios para impedir colisiones entre identificadores de tabla. Los comandos y las reglas de compatibilidad de otros productos DBWarp pertenecen a su documentación, revisada por separado, y no se duplican aquí deliberadamente.

## Límite de privacidad y revisión

Un paquete no relaja el modelo de privacidad:

- los orígenes de bases de datos en vivo siguen emitiendo identificadores
  saneados de tablas, columnas e índices;
- los valores de archivos estructurados solo se decodifican cuando se habilita
  `--measure-compression --yes`;
- las muestras decodificadas permanecen en memoria;
- los metadatos del paquete utilizan identificadores de origen y etiquetas
  elegidos por el cliente;
- ningún comando de paquetes envía telemetría ni carga archivos.

El cliente puede eliminar cualquier Blueprint secundario o entrada de origen antes
de compartir el paquete.
