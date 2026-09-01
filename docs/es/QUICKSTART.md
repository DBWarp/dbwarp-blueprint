# Inicio rápido

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../QUICKSTART.md).

**Idiomas:** [English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | **Español** | [Polski](../pl/QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [中文](../zh/QUICKSTART.md)

Este inicio rápido está dirigido a personal de ingeniería de ventas, administración de bases de datos o revisión de seguridad que necesite producir un archivo Blueprint de DBWarp que se pueda compartir sin exponer datos del cliente.

## 1. Elegir cómo ejecutar la herramienta

Utilice una de estas opciones:

- Descargar un binario de una versión publicada y verificar su suma de comprobación.
- Compilar desde el código fuente con `./build.sh`.
- Compilar desde el paquete de versión con dependencias incluidas para una revisión estricta y sin conexión de las dependencias.

Consulte [`BUILD.md`](BUILD.md) y [`binaries/README.md`](BINARIES.md).

Seleccione explícitamente un idioma de presentación cuando sea necesario:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

Los valores admitidos son `en`, `de`, `fr`, `es`, `pl`, `ja` y `zh`. El
idioma de presentación cambia la ayuda, las solicitudes, los diagnósticos, el
texto de progreso y el contenido de la presentación. Nunca cambia los nombres
de opciones, los valores aceptados, los esquemas de URI, los selectores, los
códigos DBP, las claves de auditoría ni el TOML Blueprint. Consulte
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Preparar las credenciales de forma segura

No incluya contraseñas en la URI de conexión. La herramienta rechaza las contraseñas incrustadas en la URI para evitar su exposición en la lista de procesos y en el historial del shell.

Patrón recomendado para el archivo de contraseña (el secreto se introduce sin eco y no aparece en el historial del shell):

```bash
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Si el nombre de usuario resulta difícil de codificar en una URI, guárdelo también en un archivo:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

A continuación, utilice `--user-file /etc/dbwarp/db.user`.

## 3. Ejecutar primero una simulación

Una simulación valida los argumentos y muestra la acción prevista sin conectarse:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

En el modo de presentación `--from-toml`, la simulación es una comprobación previa local y no lee la base de datos.

Para varios orígenes del cliente, ejecute en modo de simulación el manifiesto por lotes:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. Ejecutar el modo de solo catálogo

El modo de solo catálogo lee metadatos y estadísticas, pero no muestras de filas:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Utilice este modo cuando una política prohíba tomar muestras de filas o cuando desee realizar una primera revisión de seguridad.

## 5. Elegir el detalle de los artefactos no tabulares

De forma predeterminada, `--artifact-detail summary` lee catálogos no tabulares, pero no definiciones de objetos. Emite recuentos acotados y clases de requisitos externos. Use `--artifact-detail none` si la política prohíbe esos catálogos.

Para obtener una topología anónima de dependencias, use `graph`. Para obtener bandas acotadas de características del lenguaje y complejidad, use `analyzed`. Ambos requieren consentimiento explícito:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


La salida nunca contiene nombres de objetos, texto de definiciones, puntos de conexión, secretos, claves, certificados ni binarios. Consulte [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) antes de aprobar el modo graph o analyzed.

## 6. Ejecutar la medición de compresión de nivel 2

El nivel 2 lee muestras acotadas de filas en memoria, las comprime localmente, escribe únicamente proporciones resumidas y descarta los bytes de las muestras:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

Utilice el nivel 2 siempre que sea posible. Proporciona a DBWarp mejores estimaciones de los bytes transmitidos, el coste del tráfico saliente y la generación de datos sintéticos de texto y binarios.

## 7. Generar una presentación

Durante la ejecución en vivo:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

## 8. Revisar antes de compartir

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
- únicamente proporciones de compresión opcionales, no bytes de las muestras.

## 9. Entregar a DBWarp

Entrega mínima:

```text
blueprint.toml
```

Para una revisión de un cliente con varios orígenes, cree y revise un paquete empaquetado en lugar de entregar el directorio de trabajo:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Los metadatos del paquete conservan los identificadores de origen, las etiquetas y los identificadores de grupo de conjuntos de datos elegidos en el manifiesto por lotes. Utilice valores anónimos y revíselos antes de la transferencia.

Utilice `docs/BATCH_AND_BUNDLES.md` cuando el cliente tenga varias bases de datos, varios conjuntos de datos Parquet o Avro, o quiera aprobar únicamente determinados orígenes o tablas para generar pruebas de rendimiento.

Conserve estos elementos como evidencia local con acceso controlado de forma predeterminada:

```text
blueprint.audit.txt
blueprint.pptx
command-used.txt
```

Las auditorías y los comandos guardados pueden contener puntos de conexión de bases de datos, entidades autenticadas, rutas locales, datos de tiempos e identificadores de origen del manifiesto. Envíelos únicamente para una necesidad concreta de soporte a través de un canal seguro aprobado. No envíe archivos de contraseñas, claves privadas de CA, volcados del cliente ni registros de la base de datos.
