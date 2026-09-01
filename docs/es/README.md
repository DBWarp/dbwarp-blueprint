<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../../README.md) y la [política de traducciones de la documentación](../TRANSLATIONS.md).

**Idiomas:** [English](../../README.md) | [Deutsch](../de/README.md) | [Français](../fr/README.md) | **Español** | [Polski](../pl/README.md) | [日本語](../ja/README.md) | [中文](../zh/README.md)

## Qué es

DBWarp Blueprint es un recopilador de Blueprint de bases de datos que prioriza la confianza. Se ejecuta dentro de su propio entorno con PostgreSQL, MySQL o SQL Server. Lee metadatos del catálogo y, solo cuando se solicita una medición de compresión, una muestra acotada de filas. Después escribe un Blueprint estructural anonimizado de la base de datos: tamaños de tablas, recuentos de filas, familias de tipos y estructura de índices y claves foráneas.

Los identificadores se sustituyen por etiquetas anónimas con clave y no se escribe ningún valor de fila en el Blueprint. De forma predeterminada, una clave nueva local al proceso impide las comprobaciones de diccionario sin conexión; `--anonymization-key-file` permite al cliente conservar las etiquetas entre ejecuciones de comparación aprobadas. Lea [`SECURITY.md`](SECURITY.md) antes de compartir cualquier salida: explica exactamente qué divulga cada modo y qué opciones amplían esa divulgación.

La salida es un archivo de texto sin formato. Puede leer cada línea antes de decidir si desea compartirlo.

DBWarp Blueprint es gratuito y de código abierto, y se ejecuta por completo dentro de su entorno. Existe para que pueda proporcionarnos datos sobre su base de datos sin proporcionarnos su base de datos.

## Por qué ejecutarlo

Comparta con nosotros su salida Blueprint y podremos indicarle cuánto más rápido trasladaría DBWarp sus datos y cómo cambiarían los plazos de su migración, de los datos de prueba de CI/CD y de los análisis.

La distancia es el factor más importante. Cuanto más lejos deban viajar sus datos, mayor será la mejora que DBWarp puede demostrar.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zúrich, Suiza

---

`dbwarp-blueprint` es el recopilador Blueprint de DBWarp que se ejecuta en el entorno del cliente. Ejecútelo dentro del propio entorno del cliente para producir un archivo `blueprint.toml` saneado y revisable que DBWarp pueda utilizar para dimensionar migraciones, generar conjuntos de datos sintéticos y realizar la planificación previa sin recibir acceso a la base de datos, volcados, nombres de esquemas ni datos de filas.

Se conecta a PostgreSQL, MySQL o SQL Server, lee metadatos del catálogo, mide opcionalmente la compresión local a partir de una muestra acotada de filas y escribe TOML en texto sin formato. También puede derivar un Blueprint a partir de archivos Parquet o Avro locales en modo sin conexión cuando la entrada ya es un archivo de datos estructurados en lugar de una base de datos en vivo. Puede abrir la salida, revisar cada línea y decidir si desea compartirla.

Opcionalmente, `--deck blueprint.pptx` también escribe un resumen en PowerPoint del mismo Blueprint anonimizado. La presentación puede generarse durante una ejecución en vivo contra una base de datos o posteriormente a partir de un archivo TOML revisado mediante `--from-toml blueprint.toml --deck blueprint.pptx`. El generador de presentaciones está integrado en el binario de Rust y no establece ninguna conexión de red.

## Para qué sirve

DBWarp necesita información estructural suficiente para estimar y planificar una transferencia:

- número de tablas;
- recuentos aproximados de filas;
- tamaños de tablas e índices;
- familias de tipos de columnas, capacidades estructurales y prefijos de índice exactos, y
  anchuras observadas redondeadas por motivos de privacidad de forma predeterminada;
- estructura de los índices y las claves foráneas;
- recuentos de artefactos no tabulares seguros para la privacidad y requisitos previos de despliegue externo;
- resúmenes opcionales de compresión por tabla y columna a partir de una pequeña muestra local;
- evidencia opcional de RTT de la base de datos desde el entorno del cliente.

Estos datos bastan para estimar el tamaño de la transferencia, elegir un plan inicial de carga masiva de DBWarp y generar un conjunto de datos sintético representativo para pruebas de rendimiento. No bastan para reconstruir el esquema ni los datos del cliente.

## Qué no hace

`dbwarp-blueprint` no:

- envía telemetría;
- llama a servidores DBWarp;
- carga el archivo Blueprint;
- lee `~/.pgpass`, `~/.my.cnf`, credenciales de nube ni claves SSH;
- lee variables de entorno de contraseñas predeterminadas como `PGPASSWORD` o `MYSQL_PWD`;
- escribe nada salvo las salidas seleccionadas para el modo activo; el modo por lotes escribe un directorio de paquete con Blueprints secundarios, auditorías secundarias y evidencia opcional de errores;
- incluye en la salida nombres reales de tablas, columnas, índices o esquemas, nombres de objetos no tabulares, definiciones SQL, puntos de conexión externos, credenciales, claves, certificados, binarios ni valores de filas.

Las ejecuciones Blueprint en vivo abren una sesión de base de datos con el punto de conexión especificado. El DNS puede utilizar el solucionador configurado y la autenticación Kerberos/SSPI integrada puede contactar con la infraestructura de identidad. El modo por lotes repite ese límite para cada origen de base de datos. Las operaciones locales con TOML, Parquet, Avro y paquetes no abren ninguna conexión de red iniciada por la aplicación.

## Descargar o compilar

| Opción | Uso recomendado | Enlace |
|---|---|---|
| Descargar un binario | prueba rápida, llamada de ingeniería de ventas, host de pruebas aislado | [`binaries/README.md`](BINARIES.md) |
| Compilar desde un clon pequeño del código fuente | revisión de seguridad, política de producción, comprobación de reproducibilidad | [`BUILD.md`](BUILD.md) |
| Compilar desde un paquete de código fuente con dependencias incluidas | auditoría estricta de dependencias sin conexión | GitHub Releases |

La opción que prioriza la confianza es compilar desde el código fuente. El repositorio normal se mantiene pequeño y utiliza `Cargo.lock` para fijar las versiones de las dependencias. Para auditorías sin conexión más estrictas, cada versión también publica un paquete de código fuente con todas las dependencias, que contiene cada archivo de código fuente de las dependencias. Los binarios de las versiones se proporcionan por comodidad junto con sumas de comprobación SHA256.

## Inicio rápido

Elija un idioma de presentación cuando resulte útil. El inglés es el idioma
predeterminado; se incluyen catálogos completos para alemán, francés, español,
polaco, japonés y chino simplificado:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --dry-run
```

Solo se traducen la ayuda orientada a personas, las solicitudes, los
diagnósticos, el texto de progreso y las etiquetas de las presentaciones de
PowerPoint. Los nombres de comandos y opciones, los valores aceptados, los
esquemas de URI, los nombres de variables de entorno, los selectores, los
códigos DBP, las claves de auditoría y el TOML generado mantienen tokens
canónicos en inglés. De este modo, la automatización y los procedimientos de
soporte son idénticos en todos los idiomas. Consulte
[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Ejecute primero una simulación. Muestra el plan sin conectarse:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --dry-run
```

Ejecución recomendada, similar a producción, con TLS, registro de auditoría y medición de compresión:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Con `--measure-compression --yes`, la salida incluye proporciones zstd por
tabla y proyecciones de compresión por columna. Los bloques por columna se
calculan a partir de la misma muestra acotada que la proporción por tabla;
están destinados a estimar conjuntos de datos de DBWarp y no escriben en disco
los valores muestreados. El esquema v3 y versiones posteriores también emiten agregados seguros de
cardinalidad y distribución por columna, además de resúmenes inferidos de
prefijos de índice y relaciones. Las huellas temporales están acotadas en
memoria y se descartan; ni los valores ni las huellas aparecen en el TOML de
Blueprint.

Desde el esquema v4, los Blueprints también inventarían objetos no tabulares. De forma predeterminada,
`--artifact-detail summary` guarda recuentos acotados por clase de objeto y de
requisito externo sin leer definiciones. `graph` añade una topología anónima de
dependencias y `analyzed` bandas acotadas de características del lenguaje y
complejidad; ambos requieren `--yes` porque incluso un grafo anónimo puede
identificar una aplicación:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


La presencia de un artefacto es evidencia para la planificación, no una
afirmación de que DBWarp pueda recrearlo o traducirlo automáticamente. Consulte
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

### Fidelidad de longitudes de MySQL

La política `balanced` predeterminada conserva exactamente las capacidades
declaradas de caracteres/bytes y las longitudes de los prefijos de índice. Las
longitudes media y p95 de los valores muestreados utilizan intervalos de error
relativo (alrededor de un 3,2 % de error máximo, con los valores de hasta 32 bytes
conservados exactamente). Así, una clave `VARCHAR(3000)` cuyos valores suelen
tener 9 caracteres se mantiene cerca de 9 caracteres en los datos generados y,
al mismo tiempo, se conservan los límites válidos de DDL e índices del origen:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

Utilice estadísticas muestreadas exactas únicamente cuando la política permita esa precisión adicional:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Utilice `--length-fidelity strict` para conservar el redondeo más amplio y
apto para compartir del comportamiento anterior en las longitudes declaradas,
observadas y de prefijo. El modo estricto sacrifica deliberadamente la
fidelidad del conjunto de datos y de los índices y no es apto para pruebas de
rendimiento representativas del cliente. La sintaxis anterior
`--preserve-exact-lengths --yes` se mantiene como alias de compatibilidad de
`--length-fidelity exact --yes`.

Las nuevos Blueprints registran campos separados `declared_length_fidelity`,
`index_length_fidelity` y `observed_length_fidelity`. El campo heredado
`length_metadata` se mantiene por compatibilidad conservadora con consumidores
anteriores. Las capacidades de caracteres de PostgreSQL son valores exactos del
catálogo; los límites de bytes dependientes de la codificación y las longitudes
de prefijo de índice siguen sin estar disponibles.

Para una prueba de rendimiento generada que sea representativa del cliente,
`--measure-compression` no es opcional: proporciona las longitudes media y p95
observadas para que una columna indexada de varios kilobytes cuyos valores reales
solo tienen unos pocos caracteres no se genere a su capacidad máxima. El
presupuesto de tiempo predeterminado para el muestreo es de 300 segundos. Aumente
`--max-wall-secs` para esquemas muy grandes. Las herramientas de planificación
posteriores deben rechazar el Blueprint si alguna columna indexada, de anchura
variable y no vacía no ha sido muestreada. La generación de compatibilidad o de
pruebas rápidas exige entonces una anulación posterior explícita y debe marcarse
como no representativa.

A continuación, revise los archivos:

```bash
less blueprint.toml
less audit.txt
```

Si su política lo permite, comparta `blueprint.toml` con DBWarp. También puede compartir una presentación después de revisarla. Conserve el registro de auditoría como evidencia operativa con acceso controlado, salvo que un caso concreto de soporte lo requiera a través de un canal seguro aprobado; contiene detalles de puntos de conexión, identidades, rutas y tiempos.

## Modo de archivos estructurados

Si el origen ya es un archivo estructurado local, genere el TOML Blueprint sin credenciales de base de datos:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

El modo Parquet lee el pie y los metadatos de los grupos de filas. Los contenedores de objetos Avro no tienen un recuento equivalente de filas en el pie, por lo que el modo Avro recorre el contenedor para contar registros y utiliza el esquema del escritor para determinar la estructura de las columnas. Ninguno de los dos modos se conecta a una base de datos ni lee opciones de credenciales.

Si su política permite el muestreo decodificado, el modo de archivos también
puede estimar la compresión propia del transporte de DBWarp a partir de muestras
locales acotadas:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Las mismas opciones funcionan con `--from-avro`. Los valores muestreados se
codifican en memoria como `dbwarp-blueprint-rowframe-v1`; solo se escriben en el
TOML Blueprint las proporciones agregadas de compresión zstd.

## Modo por lotes y paquetes

Para varias bases de datos, varias tablas o conjuntos de datos, o la revisión del entorno de un cliente, utilice un manifiesto por lotes y escriba un directorio de paquete:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

El directorio de trabajo contiene `bundle.toml`, archivos Blueprint secundarios para cada origen y registros de auditoría por origen con acceso controlado. No transfiera todo el directorio de trabajo de forma predeterminada. Puede enumerarlo, extraerlo o crear un paquete Blueprint empaquetado y revisado por separado:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

Consulte [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md) para conocer la
sintaxis del manifiesto, los modos de conjuntos de datos de archivos
estructurados y las reglas de los selectores.

## Comandos habituales para bases de datos

PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Para consultar ejemplos de Kerberos, SSPI y Entra ID, consulte [`AUTH.md`](AUTH.md). Para CA internas, mTLS y verificación del nombre de host, consulte [`TLS.md`](TLS.md).

## Modo de solo catálogo

Si una política prohíbe muestrear filas, omita `--measure-compression`:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

El modo de solo catálogo lee únicamente metadatos. DBWarp aún puede realizar estimaciones a partir del tamaño de las tablas, los recuentos de filas, las familias de tipos y la estructura de los índices y las claves foráneas, pero la compresión y el realismo del conjunto de datos sintético son menores porque se debe inferir la entropía del texto y los datos binarios.

## Vista previa de la salida

```toml
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 6
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 28
row_count = 12500000
table_bytes = 4200000000
index_bytes = 1100000000

[tables.table-001]
rows = 12500000
table_bytes = 4200000000
index_bytes = 1100000000
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

El contrato completo del archivo se documenta en [`FORMAT.md`](FORMAT.md). El registro de auditoría se documenta en [`AUDIT.md`](AUDIT.md).

## Presentación visual de resumen

Genere una presentación durante la ejecución en vivo:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

O créela posteriormente a partir de un archivo Blueprint revisado, sin conexión a la base de datos:

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

La presentación se adapta al tamaño del esquema: detalle por tabla para esquemas pequeños, diapositivas de caracterización para esquemas grandes, resumen de compresión cuando existen datos de nivel 2 y una diapositiva sobre el modelo de confianza. Consulte [`DECK.md`](DECK.md).

## Documentación

Empiece aquí:

- [`docs/QUICKSTART.md`](QUICKSTART.md): primera ejecución segura y primer paquete de entrega.
- [`docs/COOKBOOK.md`](COOKBOOK.md): procedimientos prácticos para PostgreSQL, MySQL, SQL Server, TLS, presentaciones y flujos sin muestreo.
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md): lo que necesita saber el personal de administración de bases de datos o revisión de seguridad antes de ejecutar la herramienta.
- [`sql/grants/README.md`](../../sql/grants/README.md): scripts de concesión de privilegios mínimos que tienen en cuenta la versión y eliminación de cuentas después de la captura.
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md): errores habituales y soluciones.
- [`docs/MESSAGES.md`](MESSAGES.md): códigos estables de mensajes para operadores `DBPnnnnS`.
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md): funcionamiento del muestreo de compresión de nivel 2.
- [`docs/INDEX.md`](INDEX.md): mapa completo de la documentación.

Puntos de partida para la revisión de seguridad:

- [`SECURITY.md`](SECURITY.md): modelo de seguridad y tratamiento de credenciales.
- [`AUDIT.md`](AUDIT.md): qué se lee, escribe, consulta y registra.
- [`FORMAT.md`](FORMAT.md): campos de salida y reglas de redondeo.
- [`TLS.md`](TLS.md): comportamiento de TLS y mTLS.
- [`AUTH.md`](AUTH.md): modos de autenticación admitidos.
- [`BUILD.md`](BUILD.md): compilación desde el código fuente y verificación de versiones.
- [`DECK.md`](DECK.md): presentación opcional de resumen en PowerPoint.

## Licencia

Apache-2.0 OR MIT.
