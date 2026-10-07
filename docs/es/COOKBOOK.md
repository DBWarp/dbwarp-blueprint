# Recetario

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../COOKBOOK.md).

**Idiomas:** [English](../COOKBOOK.md) | [Deutsch](../de/COOKBOOK.md) | [Français](../fr/COOKBOOK.md) | **Español** | [Polski](../pl/COOKBOOK.md) | [日本語](../ja/COOKBOOK.md) | [中文](../zh/COOKBOOK.md)

Procedimientos orientados a tareas para flujos de trabajo habituales de `dbwarp-blueprint`.

## Procedimiento: sesión de operador localizada

Seleccione uno de los catálogos de idiomas completos integrados, manteniendo
canónicos los comandos, valores, identificadores y esquemas de salida:

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

Para ejecuciones desatendidas, establezca `DBWARP_BLUEPRINT_LANG=fr` o una
configuración regional estándar del proceso. Un `--lang` explícito siempre
tiene prioridad. Los códigos DBP y los detalles de bajo nivel del controlador
permanecen canónicos para que un error localizado se pueda buscar y compartir
con el servicio de soporte.

## Procedimiento: PostgreSQL con CA interna

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

Utilice este procedimiento para una revisión normal de PostgreSQL en producción. Si falla la verificación del nombre de host, corrija el certificado del servidor o utilice el nombre DNS correcto; no utilice `--tls-skip-verify` salvo en pruebas de bucle invertido.

## Procedimiento: MySQL con archivo de nombre de usuario

Resulta útil cuando el nombre de usuario contiene caracteres difíciles de codificar en una URI.

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

La receta anterior ya utiliza la política equilibrada predeterminada: metadatos exactos de MySQL declaration/index y anchos muestreados ajustados.

Confirme `declared_length_fidelity = "exact"`, `index_length_fidelity = "exact"` y `observed_length_fidelity = "relative-rounded-v2"`. Utilice `--length-fidelity exact --yes` solo después de que su organización apruebe el intercambio de estadísticas de longitud muestreadas exactas. Los nombres y los valores permanecen excluidos.

En bases de datos con miles de tablas, aumente `--max-wall-secs` por encima de su valor predeterminado de 300 segundos si es necesario. Los marcadores de fidelidad describen la política; no indican que el muestreo haya alcanzado todas las tablas.

## Procedimiento: autenticación SQL de SQL Server

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

Los modos TLS de SQL Server que verifican certificados utilizan el almacén de
confianza del sistema operativo cuando se omite `--tls-ca`. Un archivo `.pem` o
`.crt` proporcionado debe contener exactamente un certificado de CA y sustituye
esas raíces. Tanto `verify-ca` como `verify-full` validan el nombre de host de la
conexión.

## Procedimiento: token de SQL Server Entra ID

Genere el token fuera de la herramienta y, a continuación, entréguelo mediante un archivo:

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
install -m 600 /dev/null "$TOKEN_FILE"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

Azure SQL presenta un certificado de una CA pública, por lo que este
procedimiento deja `--tls-ca` sin establecer y utiliza el almacén de confianza
del sistema operativo. Un archivo `--tls-ca` proporcionado sustituye ese almacén
por un certificado; consulte [TLS](TLS.md).

## Procedimiento: revisión de seguridad de solo catálogo

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

Este es el modo de revisión con la menor cantidad de fricción. Evita el muestreo de filas, pero produce estimaciones de compresión y salida menos precisas.

## Evaluar la complejidad de migración no tabular

Comience con el resumen predeterminado para recopilar recuentos y requisitos externos sin leer definiciones:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```


Tras la aprobación de seguridad, recopile dependencias anónimas y evidencia acotada de complejidad del lenguaje:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```


Revise `visibility`, todas las tres opciones de integridad, `catalogs_unreadable`, `families_not_inventoried` y `counts_by_external_class`. Trate cada clase externa como una tarea de migración explícita. No considere que un objeto inventariado sea una prueba de que DBWarp puede recrearlo o traducirlo; pregunte a DBWarp qué tipos de objetos son compatibles con su migración. Consulte [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Procedimiento: deshabilitar la sonda de RTT

De forma predeterminada, la herramienta ejecuta cinco sondas `SELECT 1` después de establecer la conexión y emite un bloque `[network]`. Si el personal de administración de bases de datos prohíbe las consultas que no sean de catálogo, deshabilítela:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

La sonda de RTT nunca lee datos de filas; cada consulta devuelve el entero constante `1`.

## Procedimiento: limitar temporalmente el muestreo de compresión

En sistemas de producción grandes, mantenga conservadora la primera ejecución:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Si la salida marca muchas muestras como sesgadas o ausentes, repita la ejecución desde una réplica de lectura con un presupuesto de tiempo mayor.

## Receta: Varias bases de datos en un solo paquete.

Utilice un manifiesto por lotes cuando desee un paquete revisado para varias bases de datos.

`customer.batch.toml`:

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

Simulación:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Ejecución:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Esto escribe `bundle.toml`, un Blueprint secundario por origen y una auditoría por origen.
Las Blueprints secundarios se pueden seguir revisando de manera independiente.

## Receta: Bases de datos mixtas y archivos de data lake.

Utilice fuentes de archivos estructurados en el mismo lote cuando tenga extracciones de Parquet o Avro junto a bases de datos en vivo.

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` fusiona archivos como `merge_same_schema` y registra el modo declarado en el paquete. Mantenga los esquemas no relacionados en fuentes separadas.

## Procedimiento: extraer un único origen o una tabla de un paquete

Después de una ejecución por lotes, enumere los orígenes:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Extraiga un origen:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Extraiga una tabla de un origen:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Utilice esto cuando solo una parte de un paquete está aprobada para ser compartida.

## Receta: Empaquetar un conjunto revisado para compartir.

El directorio del paquete de trabajo contiene Blueprints secundarios y auditorías con control de acceso. No lo transfiera por completo. Después de revisar los valores del manifiesto y los Blueprints secundarios, cree un único archivo para compartir:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

El archivo empaquetado conserva los identificadores de origen, las etiquetas,
los identificadores de grupo de conjuntos de datos y los metadatos de rutas de
auditoría proporcionados por el operador. Utilice valores anónimos, inspeccione
el TOML empaquetado y transfiéralo únicamente por el canal aprobado.

## Receta: Paquete por lotes para compartir.

Siga las [directrices de revisión y compartición](QUICKSTART.md#review-and-share). Mantenga el manifiesto de trabajo, las auditorías y los registros de comandos localmente; cree este directorio separado solo a partir del Blueprint empaquetado revisado.

```text
blueprint-share/
  customer-blueprint-bundle.packed.toml
```

## Procedimiento: presentación sin conexión a partir de TOML revisado

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

Este modo solo lee el archivo TOML y escribe la presentación. Rechaza las opciones de base de datos en vivo en lugar de ignorarlas silenciosamente.

## Procedimiento: reproducibilidad idéntica byte a byte

Fije la marca de tiempo y reutilice la misma clave de anonimización protegida
que usted custodia:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

El archivo de clave debe contener exactamente 32 bytes sin procesar o 64 caracteres hexadecimales, no debe ser group/world-readable en Unix, y nunca debe compartirse. Sin esta opción, una clave generada aleatoriamente por el sistema operativo cambia intencionalmente el orden de la etiqueta anónima en cada ejecución. Fijar solo `--generated-at` es insuficiente. Utilice la receta completa para obtener instantáneas forenses aprobadas; una presentación generada dos veces a partir del mismo Blueprint revisado permanecerá idéntica en bytes cuando su marca de tiempo y lenguaje no se modifiquen.

## Receta: Paquete para compartir con DBWarp.

Siga las [directrices de revisión y compartición](QUICKSTART.md#review-and-share). El paquete predeterminado contiene solo el Blueprint aprobado:

```text
blueprint-share/
  blueprint.toml
```

Agregue `blueprint.pptx` únicamente después de una revisión y aprobación separadas. Mantenga los registros de auditoría, los registros de comandos y el material credential/key fuera del directorio compartido; envíe los registros de auditoría solo para una necesidad de soporte específica a través de un canal seguro aprobado.
