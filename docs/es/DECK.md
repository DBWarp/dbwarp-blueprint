# Presentación visual de resumen

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. El inglés es la fuente canónica y este texto no debe considerarse apto para uso contractual. Consulte el [documento canónico en inglés](../../DECK.md).

**Idiomas:** [English](../../DECK.md) | [Deutsch](../de/DECK.md) | [Français](../fr/DECK.md) | **Español** | [Polski](../pl/DECK.md) | [日本語](../ja/DECK.md) | [中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` escribe un resumen opcional en PowerPoint
(`.pptx`) del Blueprint junto al archivo TOML indicado por `--out`.
`dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` genera posteriormente
la misma presentación a partir de un archivo Blueprint existente y revisado,
sin conectarse a una base de datos. Es una presentación de los mismos datos
anonimizados: no se lee, envía ni calcula nada más sobre su base de datos.

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

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` localiza el texto de la presentación dirigido a
personas y los metadatos de idioma de PowerPoint. Los identificadores
anónimos, los nombres de tipos de base de datos, los métodos de índice, las
mediciones y el TOML de origen permanecen canónicos e independientes del
idioma. La validación se detiene y rechaza un catálogo incompleto en lugar de
sustituir por inglés una frase ausente de la presentación. Consulte
[`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## Pie de página y confidencialidad

Cada diapositiva de contenido utiliza el pie de página corporativo de DBWarp:
un logotipo pequeño a la izquierda, un separador y un nivel de confidencialidad
opcionales, un número de diapositiva centrado y sin texto adicional, y
`DBWarp.com` a la derecha. La diapositiva de título no se numera.

Use `--deck-confidentiality public|internal|confidential|restricted` para añadir
una de las etiquetas de clasificación integradas y localizadas. Cualquier otro
valor seguro y no vacío se trata como una etiqueta personalizada y se muestra
literalmente; entrecomille los valores que contengan espacios, por ejemplo
`--deck-confidentiality "CLIENT // SENSITIVE"`. Las etiquetas no pueden tener
espacios iniciales o finales, caracteres de control o de formato bidireccional,
ni superar 48 unidades de ancho de visualización. Omita la opción para no
mostrar ninguna etiqueta. La configuración solo cambia la presentación; no
modifica el archivo Blueprint ni los datos resumidos en la presentación, y
sigue siendo determinista cuando se fija `--generated-at`.

## Propiedades de confianza

- **Generada localmente y desde memoria.** La presentación se representa a
  partir del mismo Blueprint en memoria que produce `blueprint.toml`. No se ejecuta
  ninguna consulta adicional a la base de datos ni una segunda pasada por el
  catálogo. En modo `--from-toml`, el Blueprint en memoria se carga en cambio desde
  el archivo TOML revisado.
- **Sin red.** La generación de la presentación no establece ninguna conexión
  saliente de ningún tipo.
- **Sin bibliotecas de terceros.** El OOXML se crea directamente en
  `src/deck.rs`; el archivo `.pptx` es un ZIP sencillo de partes XML que puede
  abrir con `unzip` y leer. No hay automatización de PowerPoint, servicios de
  representación ni crates adicionales en el grafo de dependencias. Las
  imágenes de logotipo DBWarp aprobadas y las fuentes estáticas DM Sans están
  integradas en el binario de Rust y se escriben como partes multimedia y de
  fuente OOXML; la generación no lee ninguna ruta de activos en tiempo de
  ejecución.
- **Sin identificadores reales ni datos de filas.** Las tablas, columnas e
  índices aparecen como los mismos marcadores anónimos que en el archivo de
  Blueprint (`table-001`, `col-1`, `idx-1`, `schema-A`), y cada número conserva la
  misma precisión documentada. La presentación no contiene hechos específicos
  del cliente aparte de los incluidos en el archivo Blueprint.
- **Determinista.** Con un valor fijado de `--generated-at`, el mismo Blueprint
  produce un `.pptx` idéntico byte a byte para el mismo idioma seleccionado
  (orden fijo de partes y marcas de tiempo fijas).

## Contenido

La presentación se adapta al tamaño del esquema:

- **Título:** logotipo y lema de DBWarp, motor, versión, clase de origen, número
  de tablas y marca de tiempo de generación.
- **Resumen ejecutivo:** señales para dirección sobre escala de migración,
  concentración de datos, complejidad de relaciones y evidencia lista para
  compartir.
- **Resumen:** totales de tablas, filas, tamaño de datos y tamaño de índices,
  además de los números de columnas, índices, claves foráneas y esquemas.
- **Esquemas pequeños** (unas pocas tablas): un panel dimensionado por tabla
  (filas, bytes, tipos de columnas e índices) y un diagrama de claves foráneas.
- **Esquemas grandes:** caracterización en lugar de enumeración:
  - *Tablas más grandes*: las tablas principales por tamaño, con un resto
    `+ N more`.
  - *Composición del esquema*: distribución de tipos de columna y estadísticas
    de índices y totales.
  - *Relaciones*: número de claves foráneas, tablas conectadas frente a
    independientes y las tablas más referenciadas (nodos centrales).
- **Compresión medida** (solo nivel 2): número de tablas muestreadas, proporción
  ponderada de zstd-3, tamaño comprimido proyectado y las tablas muestreadas más
  comprimibles.
- **Modelo de confianza:** una diapositiva final que resume las garantías
  anteriores.

## Revisión del resultado

El archivo `.pptx` es un paquete OOXML estándar. Para auditar exactamente su
contenido:

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Ábralo en PowerPoint, LibreOffice Impress o Google Slides. El generador está en
[`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) y se integra en el binario de Rust. No existe
un generador de presentaciones separado que deba instalar, auditar o mantener
sincronizado.
