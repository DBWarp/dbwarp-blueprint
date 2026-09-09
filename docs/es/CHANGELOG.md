# Historial de cambios

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa y puede contener errores. No debe considerarse texto apto para uso contractual. Consulte el [documento canónico en inglés](../../CHANGELOG.md).

**Idiomas:** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | [Français](../fr/CHANGELOG.md) | **Español** | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | [简体中文](../zh/CHANGELOG.md)

Las versiones de publicación identifican al recopilador. La versión del esquema
Blueprint y la codificación de las muestras de compresión son contratos de
compatibilidad independientes; consulte [FORMAT.md](FORMAT.md) y
[medición de compresión](COMPRESSION_MEASUREMENT.md).

## 1.5.1

### Captura y fidelidad

- Conserva el esquema Blueprint v6 y distingue las estimaciones del catálogo,
  las observaciones muestreadas y la evidencia no disponible sobre la actualidad
  de las estadísticas.
- Mejora la medición de cargas binarias, el análisis de cargas ya comprimidas
  y las sondas de compresión acotadas. Las proporciones de distintos valores de
  `sample_encoding` no son intercambiables; los consumidores deben reconocer la
  codificación antes de utilizar una medición.
- Acota los reintentos adaptativos de muestreo de MySQL y SQL Server, incluidos
  los valores de tamaño excesivo y la expansión por el juego de caracteres.
  Registra el sesgo restante debido a prefijos y conserva los metadatos de
  longitud original de los valores muestreados.
- Corrige la detección de truncamiento de MySQL cuando el juego de caracteres de
  la conexión cambia la longitud de los bytes devueltos.
- Mejora el tratamiento de la longitud de valores sintéticos, la cardinalidad,
  la distribución y la localidad de los datos en el núcleo compartido de Blueprint.

### Operación y revisión

- Aclara la configuración de cuentas dedicadas con privilegios mínimos, las
  instrucciones de compilación desde el código fuente, la verificación mediante
  compilaciones comparativas y la matriz de versiones de bases de datos validadas.
- Actualiza la documentación traducida automáticamente y los textos de ejecución,
  manteniendo el inglés como referencia autoritativa y las traducciones como
  complemento.
- Refuerza las comprobaciones del modo ejecutable del código fuente público y de
  los archivos de distribución de las versiones.
- Añade el historial de versiones y las instrucciones de soporte y contribución
  a las distribuciones de código fuente y de binarios.
- Elimina ilustraciones ASCII sin uso, sin modificar los modos de banner admitidos.

### Compatibilidad y despliegue

El nuevo recopilador sigue pudiendo leer los Blueprints existentes. No se
garantiza lo contrario: el lector de la versión 1.5.0 rechaza el nuevo campo
opcional `sample_layout`, y los consumidores antiguos pueden rechazar las nuevas
codificaciones de muestras de compresión. Por tanto, disponer de un analizador de
esquema v6 no demuestra por sí solo la compatibilidad con versiones posteriores.
Actualice y valide las herramientas consumidoras junto con el recopilador antes
de utilizar nuevas capturas para presentaciones, generación o planificación
basada en compresión.

Fije el artefacto exacto de la versión y su suma de comprobación. La validación de
una versión anterior no demuestra que un binario diferente produzca resultados
idénticos.

## 1.5.0

La versión anterior proporciona captura con esquema v6 para PostgreSQL, MySQL y
SQL Server, inspección de archivos estructurados, salida local de Blueprint y
presentaciones, y scripts de concesión de permisos que tienen en cuenta la
versión. Consulte la
[etiqueta de la versión](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)
para obtener su código fuente y sus artefactos exactos.

Para informar de problemas, consulte [SUPPORT.md](SUPPORT.md). Para las
instrucciones de contribución, consulte [CONTRIBUTING.md](CONTRIBUTING.md).
