# Orígenes Blueprint a partir de archivos estructurados

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../STRUCTURED_FILES.md).

**Idiomas:** [English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | [Français](../fr/STRUCTURED_FILES.md) | **Español** | [Polski](../pl/STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | [中文](../zh/STRUCTURED_FILES.md)

`dbwarp-blueprint` puede crear un TOML Blueprint saneado a partir de entradas
Parquet y Avro locales cuando el origen ya es un archivo y no una base de datos
en vivo.

Este es un modo sin conexión:

- sin conexión a una base de datos;
- sin credenciales;
- sin telemetría;
- sin valores de filas escritos en la salida;
- los identificadores de tabla y columna solo se emiten como `table-NNN` y `col-N`;
- la auditoría solo registra las rutas de los archivos locales de entrada y
  salida, y el hash de la salida.

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

El modo Parquet lee los metadatos del pie y de los grupos de filas. Deriva:

- el número de filas a partir de los metadatos del archivo;
- las etiquetas de tipo de columna a partir de los tipos físicos y lógicos de
  Parquet;
- la nulabilidad a partir de los niveles de definición;
- las fracciones nulas observadas cuando se dispone de estadísticas completas
  de las columnas;
- la anchura media codificada aproximada y la proporción de almacenamiento del
  origen por columna a partir de los metadatos de los fragmentos de columna;
- los bytes del objeto de origen, el número de grupos de filas, el número de
  particiones y la procedencia del códec.

La captura Parquet limitada a metadatos no inventa una anchura p95 decodificada.
El muestreo decodificado opcional sustituye las pistas de anchura codificada por
observaciones decodificadas de `len_avg`, `len_p95`, `null_fraction` y
`table_bytes` lógicos.

Parquet sin muestreo decodificado usa bytes sin comprimir de los fragmentos de
columna como estimación lógica `table_bytes`. El `ratio_storage` de tabla compara
ese valor con el tamaño real del objeto; el `ratio_storage` de columna compara
bytes sin comprimir y comprimidos del fragmento. Son señales para planificar archivos, no
compresión de transporte DBWarp, y nunca se emiten como `ratio_zstd_3`.

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/customer-sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Los contenedores de objetos Avro no exponen un número de filas en un pie al
estilo de Parquet. Por ello, el modo Avro recorre el contenedor una vez para
contar los registros, derivar `table_bytes` lógicos y observar `len_avg`,
`len_p95` y `null_fraction` por columna. El esquema del escritor proporciona los
metadatos de tipo lógico. `storage_bytes` y `ratio_storage` describen el
contenedor Avro, no una estimación de transferencia de DBWarp. Esto es adecuado
para la planificación del estimador y de conjuntos de datos sintéticos.

## Fidelidad de tipos lógicos

La captura de archivos estructurados conserva los metadatos lógicos acotados
que necesita el estimador: precisión y escala decimales, familias de fecha y
hora, precisión de marcas de tiempo y semántica UTC/local, UUID, anchura binaria
fija, cadenas UTF-8 y bytes sin procesar. Los campos que solo contienen valores
nulos permanecen como `type = "null"` en lugar de convertirse en texto sintético.

Las hojas Parquet anidadas y los arrays, mapas, registros o uniones de varios
tipos de Avro no pueden representarse como un único escalar SQL exacto. El
Blueprint registra un tipo `json` normalizado y un valor `source_semantics` como
`"repeated-leaf"`, `"nested-json"` o `"multi-type-union"`. Los generadores
posteriores deben identificar estos valores como presión JSON representativa,
sin afirmar una ida y vuelta exacta del esquema anidado.

Las raíces de nombres de archivo, las rutas Parquet, los nombres de campos Avro
y las etiquetas `logical_table` de un lote no se escriben como identificadores
Blueprint. Un conjunto de varios archivos emite identificadores `table-NNN`
deterministas, agrega bytes de objetos, particiones, grupos de filas, códecs,
anchuras, fracciones nulas y procedencia de compresión compatible, y rechaza los
archivos cuyos contratos lógicos de columnas difieren.

## Muestreo de compresión decodificada

El modo de archivos estructurados admite un muestreo opcional de compresión
decodificada:

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Las mismas opciones funcionan con `--from-avro`.

Cuando se habilita, `dbwarp-blueprint`:

- decodifica hasta `--sample-rows` registros del archivo;
- codifica los valores muestreados mediante la misma trama de fila
  `dbwarp-blueprint-rowframe-v1` que utiliza la captura de Blueprints de bases de datos
  en vivo;
- emite resúmenes de compresión zstd-3 por tabla y por columna;
- registra `sample_encoding = "dbwarp-blueprint-rowframe-v1"` en el TOML generado;
- conserva los bytes muestreados solo en memoria y nunca escribe valores de
  filas en disco.

`--measure-compression` requiere `--yes` porque lee valores decodificados del
cliente, aunque solo conserva proporciones agregadas.

El muestreador actual utiliza una muestra determinista de los primeros N
registros. Es reproducible y barato, pero puede estar sesgado si un archivo está
ordenado o agrupado. Para estimaciones críticas, prefiera un archivo
representativo o genere varios archivos Blueprint a partir de fragmentos
distintos. Una versión futura puede incorporar muestreo estratificado por
grupos de filas o bloques.

## Alcance

El modo Blueprints a partir de archivos estructurados resulta útil para:

- dimensionar una importación Parquet/Avro antes de una ejecución de DBWarp;
- generar un conjunto de datos sintético neutro respecto al cliente a partir de
  metadatos de archivos;
- planificar flujos Parquet/Avro -> DBWarp columnar -> target database.

No sustituye a la captura de Blueprints de bases de datos en vivo cuando el origen
real es una base de datos compatible, es decir, PostgreSQL, MySQL o SQL Server. Un
catálogo de base de datos contiene detalles de índices, claves, claves
foráneas, actualidad de las estadísticas y disposición del motor que no están
presentes en metadatos de archivo genéricos.
