# Inicio rápido

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../QUICKSTART.md).

**Idiomas:** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | **Español** | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [中文](../zh/QUICKSTART.md)

Esta guía de inicio rápido es para un administrador de bases de datos (DBA) o un revisor de seguridad que necesita generar un archivo DBWarp Blueprint que se pueda compartir sin exponer datos.

## 1. Elegir cómo ejecutar la herramienta

Utilice una de estas opciones:

- Descargar un binario de una versión publicada y verificar su suma de comprobación.
- Compilar desde el código fuente con `./build.sh`.
- Compilar desde el paquete de versión con dependencias incluidas para una revisión estricta y sin conexión de las dependencias.

Consulte [`BUILD.md`](BUILD.md) y [`binaries/README.md`](BINARIES.md).

Seleccione explícitamente un idioma de presentación cuando sea necesario:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

Los valores admitidos son `en`, `de`, `fr`, `es`, `pl`, `ja` y `zh`. El
idioma de presentación cambia la ayuda, las solicitudes, los diagnósticos, el
texto de progreso y el contenido de la presentación. Nunca cambia los nombres
de opciones, los valores aceptados, los esquemas de URI, los selectores, los
códigos DBP, las claves de auditoría ni el TOML Blueprint. Consulte
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Aprovisionar una cuenta dedicada con privilegios mínimos

Haga esto antes de cualquier conexión real, incluidos los ejemplos con
`--dry-run` que más adelante se convertirán en capturas. No empiece con una
cuenta propietaria de la aplicación, administradora, superusuaria, `root`,
`sa` ni `db_owner`.

1. Identifique el motor y la versión exactos, la base de datos y los esquemas
   aprobados.
2. Elija el nivel de captura: `basic` solo para catálogos de tablas, `standard` para agregar una muestra de filas limitada, o `enhanced` para analizar también objetos que no son tablas.
3. Pida al DBA que copie el script correspondiente de
   `sql/grants/<engine>/`, edite todos los valores marcados de base de datos,
   esquema, principal, contraseña y selector de rol, y lo ejecute mediante el
   proceso normal de control de cambios.
4. Use la cuenta dedicada que crea y pase el mismo alcance aprobado con una
   opción `--schema NAME` por esquema en cada comando real.
5. Después de revisar la captura, pida al administrador de la base de datos (DBA) que revise y ejecute, bajo `sql/revoke/`, el script de revocación correspondiente al motor seleccionado para eliminar la cuenta y los permisos.

Los scripts distinguen deliberadamente entre permisos de alcance exacto y
roles integrados más cómodos, y explican cuándo un rol es más amplio. Consulte
[`../../sql/grants/README.md`](../../sql/grants/README.md) para ver los scripts
ejecutables y
[`../../sql/grants/DATABASE_PERMISSIONS.md`](../../sql/grants/DATABASE_PERMISSIONS.md)
para conocer la justificación por versión destinada al DBA y al personal de
seguridad. El colector no crea, amplía ni elimina por sí mismo principales de
base de datos.

## 3. Preparar las credenciales de forma segura

No incluya contraseñas en la URI de conexión. La herramienta rechaza las contraseñas incrustadas en la URI para evitar su exposición en la lista de procesos y en el historial del shell.

Patrón recomendado para el archivo de contraseña (el secreto se introduce sin eco y no aparece en el historial del shell):

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Si el nombre de usuario resulta difícil de codificar en una URI, guárdelo también en un archivo:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

A continuación, utilice `--user-file /etc/dbwarp/db.user`.

## 4. Ejecutar primero una simulación

Una simulación valida los argumentos y muestra la acción prevista sin conectarse:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

En el modo de presentación `--from-toml`, la simulación es una comprobación previa local y no lee la base de datos.

Para múltiples orígenes, ejecute una prueba simulada del manifiesto del lote en su lugar:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. Ejecutar el modo de solo catálogo

El modo de solo catálogo lee metadatos y estadísticas, pero no muestras de filas:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Utilice este modo cuando una política prohíba tomar muestras de filas o cuando desee realizar una primera revisión de seguridad.

## 6. Elegir el detalle de los artefactos no tabulares

De forma predeterminada, `--artifact-detail summary` lee catálogos no tabulares, pero no definiciones de objetos. Emite recuentos acotados y clases de requisitos externos. Use `--artifact-detail none` si la política prohíbe esos catálogos. La sonda de topología de solo recuento se sigue ejecutando; consulte la [referencia de permisos](../../sql/grants/README.md#topology-evidence).

Para obtener una topología anónima de dependencias, use `graph`. Para obtener bandas acotadas de características del lenguaje y complejidad, use `analyzed`. Ambos requieren consentimiento explícito:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


La salida nunca contiene nombres de objetos, texto de definiciones, puntos de conexión, secretos, claves, certificados ni binarios. Consulte [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) antes de aprobar el modo graph o analyzed.

## 7. Ejecutar la medición de compresión de nivel 2

El nivel 2 lee muestras acotadas de filas en memoria, calcula mediciones
agregadas de compresión, densidad de valores NULL, cardinalidad/frecuencia,
longitud y estilo, y descarta los valores muestreados:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

Utilice el Nivel 2 siempre que sea posible. Proporciona estimaciones más precisas del tamaño de la transferencia y del costo de salida.

## 8. Generar una presentación

Durante la ejecución en vivo:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

O después de la revisión, sin conexión a la base de datos:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 9. Revisar antes de compartir

Revise:

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

Propiedades esperadas:

- ningún nombre real de tabla;
- ningún nombre real de columna;
- ningún valor de fila;
- ningún comentario salvo la cabecera fija;
- recuentos y tamaños en bytes redondeados;
- identificadores anonimizados como `table-001`, `col-1` y `schema-A`;
- recuentos de artefactos acotados y, si se aprueba, identificadores anónimos de artefactos;
- evidencia explícita de artefactos incompletos o ilegibles en lugar de omisiones silenciosas;
- mediciones agregadas opcionales de compresión, densidad de valores NULL,
  cardinalidad/frecuencia, longitud y estilo, nunca valores muestreados.

## 10. Compartir con DBWarp.

Mínimo para compartir:

```text
blueprint.toml
```

Para múltiples orígenes, cree e inspeccione un paquete (bundle) en lugar de compartir el directorio de trabajo:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Los metadatos del paquete conservan los identificadores de origen, las etiquetas y los identificadores de grupo de conjuntos de datos elegidos en el manifiesto por lotes. Utilice valores anónimos y revíselos antes de la transferencia.

Consulte [Paquetes de recopilación por lotes y planos](BATCH_AND_BUNDLES.md) si tiene varias bases de datos o varios conjuntos de datos Parquet o Avro, o si desea compartir solo fuentes o tablas seleccionadas.

### Revisar y compartir

Comparta por defecto solo el `blueprint.toml` revisado o el paquete empaquetado. Una presentación solo puede acompañarlo tras revisar su contenido y nivel de confidencialidad y aprobarla por separado conforme a la política de su organización.

Mantenga las auditorías, los registros de comandos y las presentaciones no aprobadas localmente y con acceso controlado. Pueden contener puntos finales, usuarios autenticados, rutas locales, datos de tiempo y identificadores de manifiestos. Envíelos solo para una necesidad de soporte específica a través de un canal seguro aprobado. Nunca incluya archivos de contraseñas o tokens, claves de anonimización, claves privadas de CA, volúmenes de bases de datos ni registros de bases de datos con un Blueprint compartido.
