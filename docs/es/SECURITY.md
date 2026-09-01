# Modelo de seguridad

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../../SECURITY.md).

**Idiomas:** [English](../../SECURITY.md) | [Deutsch](../de/SECURITY.md) | [Français](../fr/SECURITY.md) | **Español** | [Polski](../pl/SECURITY.md) | [日本語](../ja/SECURITY.md) | [中文](../zh/SECURITY.md)

`dbwarp-blueprint` tiene modos independientes para bases de datos en vivo, archivos estructurados, procesamiento por lotes, paquetes y presentaciones. El modo seleccionado determina su ámbito de red y sistema de archivos. No tiene telemetría, comprobación de actualizaciones, comprobación de licencias, llamadas de analítica ni rutas de carga.

Esta página explica los límites de seguridad para que su equipo pueda decidir si ejecuta la herramienta.

## Notificación de vulnerabilidades

Informe en privado de las posibles vulnerabilidades mediante la
[notificación privada de vulnerabilidades de GitHub](https://github.com/DBWarp/dbwarp-blueprint/security/advisories/new).
No incluya detalles sensibles para la seguridad en una incidencia pública.
Incluya la versión exacta, el sistema operativo, los pasos de reproducción y la
mínima evidencia segura necesaria para evaluar el informe.

## Red

| Modo | Uso de la red durante la ejecución |
|---|---|
| `--connect` en vivo | Una sesión del controlador de base de datos con el punto de conexión indicado. La resolución DNS puede contactar con el solucionador configurado. La autenticación Kerberos/SSPI integrada también puede contactar con infraestructura de identidad configurada, como un KDC o un controlador de dominio. |
| `--batch-manifest` | Una sesión del controlador por cada origen de base de datos del manifiesto, procesados secuencialmente. Los orígenes Parquet y Avro locales no utilizan la red. Se siguen aplicando las salvedades anteriores sobre DNS y autenticación integrada. |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Ninguna conexión de red iniciada por la aplicación. Las entradas en sistemas de archivos de red siguen dependiendo del sistema operativo y del almacenamiento. |

La herramienta no llama a ningún servicio de DBWarp ni a ninguna API de nube. Los controladores de bases de datos y el sistema operativo del host pueden generar el tráfico de soporte de protocolos descrito anteriormente.

`--max-wall-secs` establece dos protecciones independientes. PostgreSQL usa un
`statement_timeout` local a la sesión y MySQL usa `max_execution_time` local a
la sesión para las sentencias `SELECT` de solo lectura del recopilador. SQL
Server no tiene un ajuste de sesión equivalente para el tiempo total transcurrido
de una sentencia, por lo que el recopilador establece `LOCK_TIMEOUT` local a la
sesión para limitar las esperas de bloqueo y conserva el plazo del cliente para
otros bloqueos. Si vence ese plazo del cliente, la herramienta cierra su
conexión; no afirma que SQL Server haya confirmado una cancelación en el
servidor. Confirme que el trabajo del servidor se detuvo antes de reintentarlo.

## Archivos leídos

Durante la ejecución, la herramienta solo lee las entradas seleccionadas en la línea de comandos o referenciadas por una entrada por lotes o de paquete:

| Archivo | Cuándo |
|---|---|
| `--user-file` | fuente del nombre de usuario |
| `--password-file` | fuente de la contraseña |
| `--anonymization-key-file` | clave HMAC opcional custodiada por el cliente para conservar etiquetas anónimas de objetos entre ejecuciones aprobadas; en Unix, el modo no debe permitir la lectura al grupo ni a otros usuarios |
| `--azure-token-file` | fuente del token de SQL Server Entra ID |
| `--tls-ca` | paquete de CA de confianza |
| `--tls-cert` | certificado TLS de cliente |
| `--tls-key` | clave privada TLS de cliente |
| `--from-toml` | archivo TOML existente de dbwarp-blueprint utilizado para crear una presentación sin conexión |
| `--from-parquet` | metadatos del archivo Parquet y, con consentimiento explícito para el muestreo, filas decodificadas acotadas |
| `--from-avro` | metadatos y registros del contenedor de objetos Avro; es necesario recorrer el contenedor para contar los registros |
| `--batch-manifest` | manifiesto por lotes y todos los archivos estructurados locales, archivos de credenciales, archivos de tokens y archivos TLS que referencia |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | TOML del paquete y los archivos Blueprint relativos necesarios para la operación seleccionada |
| `/dev/tty` | solicitud interactiva de contraseña en sistemas tipo Unix |

No lee `~/.pgpass`, `~/.my.cnf`, archivos de credenciales de nube, claves SSH, el historial del shell ni variables de entorno de contraseñas predeterminadas.

Para PostgreSQL y MySQL, un paquete PEM proporcionado mediante `--tls-ca`
sustituye los certificados raíz de Mozilla integrados. SQL Server utiliza el
almacén de confianza del sistema operativo cuando se omite `--tls-ca`; un
archivo `.pem` o `.crt` proporcionado debe contener exactamente un certificado
de CA y sustituye esas raíces. SQL Server valida el nombre de host en ambos
modos de verificación de certificados y rechaza `--tls-cert`/`--tls-key` con
`DBP1015E`, ya que su controlador no implementa la autenticación mediante
certificado de cliente.

## Archivos escritos

Durante la ejecución, la herramienta puede escribir:

| Archivo | Cuándo |
|---|---|
| `--out` | salida Blueprint para los modos de base de datos en vivo, archivo estructurado, extracción de paquete o empaquetado de paquete |
| `--deck` | resumen opcional en PowerPoint (.pptx), generado localmente a partir del Blueprint anonimizado o de la entrada `--from-toml` (sin una lectura adicional de la base de datos, sin red y sin biblioteca de terceros) |
| `--audit-log` | copia opcional del registro de auditoría |
| `--out-dir` | directorio por lotes que contiene `bundle.toml`, `blueprints/*.blueprint.toml`, `audits/*.audit.txt`, un marcador de propiedad y `errors.txt` cuando falla uno o más orígenes; durante la publicación atómica se utiliza un directorio de preparación adyacente que se elimina tras un error controlado |

El registro de auditoría también se imprime en stderr.

Trate cada auditoría y cada archivo por lotes `errors.txt` como evidencia operativa con acceso controlado. Pueden contener nombres de puntos de conexión, rutas locales, identificadores de origen del manifiesto, errores del controlador y datos de tiempos. Para SQL Server, la auditoría incluye el inicio de sesión autenticado exacto
(`ORIGINAL_LOGIN()`), la entidad de seguridad efectiva del servidor
(`SUSER_SNAME()`) y la entidad de seguridad de la base de datos (`USER_NAME()`),
además de una entidad esperada opcional y el resultado de la aserción. Estas
identidades no se escriben en un Blueprint de un solo origen ni en una presentación. Los metadatos del paquete conservan los identificadores de origen, las etiquetas y los identificadores de grupo de conjuntos de datos proporcionados por el operador; elija valores anónimos y revise el TOML del paquete antes de transferirlo.

## Variables de entorno

De forma predeterminada, no se lee ninguna variable de entorno durante la ejecución para obtener credenciales.

Si utiliza `--password-env NAME`, `--user-env NAME` o `--azure-token-env NAME`, la herramienta lee exactamente la variable indicada. No recurre a valores predeterminados habituales como `PGPASSWORD`, `MYSQL_PWD` o `MSSQL_PASSWORD`.

## Credenciales

Las credenciales se encapsulan en un tipo `Secret` que deliberadamente no implementa `Debug`, `Display`, `Clone` ni serialización. Esto dificulta que un registro accidental llegue a compilarse.

Las credenciales se entregan al controlador de la base de datos únicamente para establecer la conexión. No se escriben en el archivo de salida ni en el registro de auditoría. El registro de auditoría conserva la fuente de las credenciales, como `file:/etc/dbwarp/db.pass`, pero no el valor.

## Patrones de credenciales rechazados

Se rechazan las contraseñas incrustadas en la URI de conexión. Por ejemplo, no se acepta:

```text
postgresql://user:password@host/db
```

Utilice en su lugar `--password-file`, `--password-env` o la solicitud interactiva. Esto evita exponer contraseñas a través del historial del shell, las listas de procesos o el desplazamiento de la terminal.

## Seguridad de la salida

El archivo Blueprint está diseñado para que pueda leerse y revisarse por una persona:

- los identificadores reales se sustituyen por nombres anónimos con clave como `table-001` y `col-1`
- los valores numéricos se redondean a intervalos documentados
- los comentarios son fijos y no se utilizan como canal de datos
- nunca se emiten valores de filas
- cuando se habilitan muestras de compresión, se comprimen localmente y se descartan

El nivel 2 en vivo aplica un límite estricto de 16 MiB de carga proyectada por
tabla antes de que el controlador de base de datos reciba los datos de filas.
Reduce el número de filas solicitado para tablas extremadamente anchas y
proyecta las celdas de anchura variable mediante truncamiento nativo del motor
en el servidor. Las sondas de estilo se limitan por separado en su proyección
SQL. El codificador local de tramas de filas aplica de forma independiente el
mismo límite por tabla. Esto evita que un valor pequeño de `--sample-rows`
transfiera una carga LOB sin límite; también significa que los valores muy
grandes solo aportan sus prefijos acotados a las estimaciones de compresión y
longitud.

El orden de tablas, esquemas, índices y objetos no tabulares utiliza
HMAC-SHA256 con separación por dominio. De forma predeterminada, la herramienta
obtiene del sistema operativo una clave nueva local al proceso y nunca la emite,
lo que impide que un lector sin conexión compruebe nombres de origen candidatos.
Utilice `--anonymization-key-file` solo cuando las mismas etiquetas anónimas
deban mantenerse entre ejecuciones de comparación aprobadas. El archivo debe
contener exactamente 32 bytes sin procesar o 64 caracteres hexadecimales y debe
protegerse como una credencial. La auditoría registra si se utilizó una clave
efímera o custodiada por el cliente, nunca el valor de la clave.

Esto reduce el riesgo de divulgación, pero no hace que todas las salidas sean seguras para cualquier destinatario. La forma anónima del esquema, los grafos de dependencias, las versiones de motores, los campos exactos opcionales y las distribuciones de tamaños inusuales pueden identificar una carga de trabajo. Revise las salidas Blueprint y de paquetes conforme a la política de clasificación de datos de su organización antes de compartirlas. No envíe auditorías ni `errors.txt` como si fueran Blueprints anonimizados.

Consulte [`FORMAT.md`](FORMAT.md) para conocer los campos exactos.

## Registro de auditoría

Cada ejecución emite un registro de auditoría que enumera:

- el punto de conexión de la base de datos contactado
- la fuente de credenciales utilizada
- las entidades de seguridad de SQL Server correspondientes a la identidad
  autenticada, al servidor efectivo y a la base de datos cuando la sesión
  pueda comunicarlas
- el modo TLS
- los archivos leídos
- los archivos escritos
- las consultas ejecutadas
- si se habilitó el muestreo de filas
- el resultado final

Consulte [`AUDIT.md`](AUDIT.md).

## Puntos de partida para revisar el código fuente

Para una revisión específica:

- `src/secret.rs`: contenedor de credenciales
- `src/main.rs`: CLI, controles de consentimiento y emisión de auditoría
- `src/audit.rs`: representación del registro de auditoría
- `src/format.rs`: formato de salida anonimizado
- `src/tls.rs`: configuración TLS
- `src/engine_pg.rs`, `src/engine_mysql.rs`, `src/engine_mssql.rs`: lectores de catálogo específicos de cada base de datos
