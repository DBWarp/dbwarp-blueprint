# Historial de cambios

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa y puede contener errores. No debe considerarse texto apto para uso contractual. Consulte el [documento canónico en inglés](../../CHANGELOG.md).

**Idiomas:** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | [Français](../fr/CHANGELOG.md) | **Español** | [Polski](../pl/CHANGELOG.md) | [日本語](../ja/CHANGELOG.md) | [简体中文](../zh/CHANGELOG.md)

Las versiones de publicación identifican al recopilador. La versión del esquema
Blueprint y la codificación de las muestras de compresión son contratos de
compatibilidad independientes; consulte [FORMAT.md](FORMAT.md) y
[medición de compresión](COMPRESSION_MEASUREMENT.md).

## 1.6.0

### Esquema Blueprint v7

- Emite el esquema v7 con clasificación explícita de tablas, organización del
  almacenamiento, particionado, estado de los segmentos, ordinales de columnas
  y semántica más completa de tipos, nulabilidad, valores generados e identidad.
- Añade evidencia por tabla y para toda la captura sobre estructura, recuentos
  de filas, bytes asignados, estadísticas y observaciones del entorno de
  origen. La evidencia ausente, denegada, mal formada o limitada por la
  selección permanece explícita en lugar de representarse como un cero medido
  o un inventario completo.
- Añade al inventario de artefactos el estado de requisitos por objeto basado
  en el catálogo y evidencia acotada de complejidad. La completitud agregada
  permanece falsa cuando no puede demostrarse que un catálogo requerido o la
  población de evaluación seleccionada estén completos.
- Mantener la compatibilidad con las versiones de esquema desde schema-v1 hasta schema-v6. Las versiones anteriores podrían no leer archivos schema-v7.
### Fidelidad y seguridad de la captura

- Mantiene separadas las lecturas acotadas completas, las muestras parciales y
  las estimaciones del catálogo en PostgreSQL, MySQL y SQL Server, incluidos
  los límites de seguridad por filas y herencia, las ventanas de rango
  adaptativas de MySQL y las tablas optimizadas en memoria de SQL Server.
- Refuerza la coherencia de cardinalidad, recuentos NULL, particiones,
  relaciones y agregados para impedir que la evidencia redondeada o incompleta
  se convierta en una afirmación exacta.
- Informa de la cobertura de tablas externas de SQL Server como una limitación
  explícita cuando PolyBase no está instalado.
- Mantiene la captura acotada por límites de filas, bytes, valores y tiempo.
  La degradación no fatal permanece visible en el Blueprint y en la auditoría
  mediante códigos de mensaje estables.

### Límite del alcance de Oracle

- Esta versión agrega la captura de catálogo Oracle Basic para Oracle 12c, 19c,
  21c y 23ai/26ai como una vista previa: solo captura el catálogo y no lee filas
  de tablas; está habilitada mediante un mecanismo de confirmación. Consulte
  `sql/grants/ORACLE_PREVIEW.md` para obtener información sobre las limitaciones.

### Artefactos de publicación y autenticación

- Los archivos de publicación de Linux incluyen la autenticación
  Kerberos/GSSAPI de SQL Server. Cargan el entorno de ejecución Kerberos de la
  plataforma solo cuando se selecciona la autenticación integrada, por lo que
  el recopilador se inicia sin bibliotecas Kerberos; la falta del entorno de
  ejecución se informa con `DBP1604E` solo para la autenticación integrada. Los
  binarios de publicación de Windows siguen incluyendo la autenticación SSPI
  de SQL Server.
- Ambos modos integrados utilizan la credencial del sistema operativo y
  rechazan la conexión cuando el principal del servidor no coincide con el
  esperado.

### Operación y compatibilidad

- Los scripts de permisos Enhanced para SQL Server añaden un permiso a nivel de
  servidor en un lote separado: `VIEW SERVER STATE` para SQL Server 2019 y
  `VIEW SERVER PERFORMANCE STATE` para 2022 y 2025. Esto permite que una captura
  Enhanced con `--artifact-detail graph` o `analyzed` informe de bandas
  aproximadas de CPU y memoria para un servidor autogestionado. Basic y
  Standard no lo otorgan e informan de esas bandas como desconocidas; un DBA
  puede eliminar el lote para conservar Enhanced sin ese permiso.
- Actualiza la pila de dependencias de SQL Server, PostgreSQL, Parquet y la
  autenticación de Windows a versiones que corrigen avisos de seguridad
  publicados. El controlador de SQL Server pasa a la versión 0.13; `--tls-ca`
  conserva su significado restrictivo y solo confía en la CA proporcionada.
  `--max-wall-secs` sigue siendo el único plazo para toda la captura.
- SQL Server solo lee la capacidad del sistema operativo cuando se solicita el
  análisis de objetos que no son tablas (`--artifact-detail graph` o
  `analyzed`). Las demás capturas registran las bandas de capacidad como no
  solicitadas en lugar de como una lectura fallida.
- La documentación en inglés sigue siendo la referencia. El Markdown traducido
complementario y contiene su propio aviso de traducción.

### Presentación

- Añade a la presentación una diapositiva de objetos que no son tablas y otra
  diapositiva independiente sobre la complejidad de los artefactos. La
  diapositiva de complejidad solo aparece cuando se capturó la complejidad y
  muestra la cobertura junto a cada banda, de modo que la evidencia incompleta
  nunca se muestra como complejidad baja.

## 1.5.1

### Captura y fidelidad

- Conserva el esquema Blueprint v6 y distingue las estimaciones del catálogo,
  las observaciones muestreadas y la evidencia no disponible sobre la actualidad
  de las estadísticas.
- Mejora la medición de cargas binarias, el análisis de cargas ya comprimidas
pruebas de compresión con límites definidos. Las relaciones obtenidas de diferentes valores de `sample_encoding` no son intercambiables; verifique la codificación antes de comparar las relaciones.
- Acota los reintentos adaptativos de muestreo de MySQL y SQL Server, incluidos
  los valores de tamaño excesivo y la expansión por el juego de caracteres.
  Registra el sesgo restante debido a prefijos y conserva los metadatos de
  longitud original de los valores muestreados.
- Corrige la detección de truncamiento de MySQL cuando el juego de caracteres de
  la conexión cambia la longitud de los bytes devueltos.

### Operación y revisión

- Aclara la configuración de cuentas dedicadas con privilegios mínimos, las
verificación de la compilación comparativa y las versiones de la base de datos admitidas.
- Actualiza la documentación traducida automáticamente y los textos de ejecución,
  manteniendo el inglés como referencia autoritativa y las traducciones como
  complemento.
- Añade el historial de versiones y las instrucciones de soporte y contribución
  a las distribuciones de código fuente y de binarios.
### Compatibilidad.

Los Blueprints existentes siguen siendo legibles por el nuevo recolector. Lo contrario no está garantizado: la versión 1.5.0 no acepta el nuevo campo opcional `sample_layout`, y las versiones anteriores pueden rechazar las nuevas codificaciones de muestra de compresión. Utilice la misma versión para generar un archivo y para crear una presentación a partir de él.

Fija la versión exacta del artefacto y el valor hash. Los resultados de una versión no están garantizados de coincidir con otra.

## 1.5.0

La versión anterior proporciona captura con esquema v6 para PostgreSQL, MySQL y
SQL Server, inspección de archivos estructurados, salida local de Blueprint y
presentaciones, y scripts de concesión de permisos que tienen en cuenta la
versión. Consulte la
[etiqueta de la versión](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)
para obtener su código fuente y sus artefactos exactos.

Para informar de problemas, consulte [SUPPORT.md](SUPPORT.md). Para las
instrucciones de contribución, consulte [CONTRIBUTING.md](CONTRIBUTING.md).
