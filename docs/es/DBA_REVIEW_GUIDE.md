# Guía de revisión para administradores de bases de datos

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../DBA_REVIEW_GUIDE.md).

**Idiomas:** [English](../DBA_REVIEW_GUIDE.md) | [Deutsch](../de/DBA_REVIEW_GUIDE.md) | [Français](../fr/DBA_REVIEW_GUIDE.md) | **Español** | [Polski](../pl/DBA_REVIEW_GUIDE.md) | [日本語](../ja/DBA_REVIEW_GUIDE.md) | [中文](../zh/DBA_REVIEW_GUIDE.md)

Esta guía está dirigida a personal de administración de bases de datos y revisión de seguridad que deba decidir si ejecuta `dbwarp-blueprint` en un entorno de producción o similar a producción.

## Modelo de ejecución

`dbwarp-blueprint` es un binario local de línea de comandos. En modo en vivo abre una conexión de base de datos a la URI que usted proporcione y escribe un archivo TOML local. No se comunica con infraestructura de DBWarp, API de nube, puntos de conexión de telemetría, servidores de licencias ni servidores de actualizaciones.

En el modo de presentación `--from-toml` no se conecta en absoluto a una base de datos.

## Cuenta recomendada

Utilice una cuenta específica con pocos privilegios y acceso de lectura a los metadatos del catálogo y, si se habilita la compresión de nivel 2, permiso para muestrear filas de tablas de usuario.

Propiedades recomendadas:

- sin privilegios de escritura;
- sin privilegios de DDL, salvo que la revisión apruebe explícitamente la
  captura MySQL mejorada, cuyos privilegios de metadatos `TRIGGER` y `EVENT`
  permiten operaciones DDL;
- sin rol de superusuario o administrador;
- acceso de lectura limitado a la base de datos que se evalúa;
- contraseña o token suministrado mediante archivo o solicitud interactiva, no incrustado en la URI.

Los permisos exactos varían según el motor y su política. Si la cuenta no puede leer algunas vistas del catálogo ni muestrear algunas tablas, la herramienta falla claramente o genera un Blueprint reducido; mantenga el registro de auditoría.

Utilice los scripts que tienen en cuenta la versión y las salvedades de
[`../../sql/grants/README.md`](../../sql/grants/README.md). Después de la captura
aprobada, elimine la cuenta dedicada del recopilador con el script
correspondiente de `sql/revoke/`; revise la base de datos, el patrón de host, el
rol y los destinos de inicio de sesión exactos antes de ejecutarlo.

## Nivel 1: solo metadatos (sin muestreo de filas)

El nivel 1 es el valor predeterminado cuando no se utiliza `--measure-compression`.

Lee:

- la versión del motor;
- la lista de tablas y las entradas de ordenación anonimizadas;
- recuentos aproximados de filas;
- tamaños de tablas e índices;
- familias de tipos de columnas, posibilidad de valores nulos y estadísticas de longitud redondeadas cuando están disponibles;
- tipo de índice, unicidad y ordinales de columnas anonimizados;
- estructura del grafo de claves foráneas cuando está disponible;
- bandas aproximadas de capacidad del origen que devuelve el punto de conexión de la base de datos cuando están disponibles;
- recuentos acotados de objetos no tabulares y requisitos externos obtenidos
  de los catálogos de objetos con el valor predeterminado
  `--artifact-detail summary` (sin definiciones);
- opcional, a menos que se configure `--no-rtt-probe`.

No lee valores de filas.

## Entorno de origen

El bloque `[source_environment]` del esquema v7 se deriva exclusivamente de
valores devueltos por la conexión de base de datos seleccionada. El recopilador
nunca inspecciona su propio host ni presenta esa estación de trabajo como el
servidor de base de datos.

PostgreSQL y MySQL exponen una configuración de búfer de base de datos con los permisos mínimos normales, por lo que la memoria es evidencia parcial con base `database-buffer-cache` y la CPU sigue siendo desconocida.

SQL Server solicita la capacidad del entorno de origen únicamente con `--artifact-detail graph` o `analyzed`, los modos de nivel avanzado. Los modos básico y estándar no realizan una consulta de capacidad del sistema operativo y registran las bandas de capacidad como `not-requested`.

El script mejorado otorga los permisos `VIEW SERVER STATE` (2019) o `VIEW SERVER PERFORMANCE STATE` (2022/2025) requeridos a nivel de servidor en un lote separado que un administrador de bases de datos (DBA) puede eliminar. Si una captura mejorada no puede leer la DMV, la captura continúa y registra el catálogo como no legible en lugar de utilizar valores locales de la máquina o inventar capacidades.

Esta ruta de captura no contacta ninguna API de nube, Kubernetes, hipervisor o
sistema operativo.

## Inventario de artefactos no tabulares

Los planos (blueprints) inventarían los objetos que no son tablas de forma independiente del muestreo de filas. El `--artifact-detail summary` predeterminado lee los catálogos de objetos pero no las definiciones y emite solo recuentos limitados y clases de prerrequisitos externos.

`--artifact-detail graph --yes` añade identificadores de objeto anónimos y aristas de dependencia. `--artifact-detail analyzed --yes` también lee temporalmente las definiciones disponibles y solo emite bandas léxicas acotadas de características y complejidad. Nunca se serializan texto de definiciones, nombres de objetos de origen, puntos de conexión, cadenas de proveedor, entidades de seguridad, secretos, claves, certificados, nombres de paquetes ni binarios.

Los privilegios de catálogo afectan a las afirmaciones de ausencia. Revise
`visibility`, `inventory_complete`, `dependencies_complete`,
`requirements_complete`, `catalogs_unreadable` y `families_not_inventoried`;
no interprete un recuento cero ni una lista de requisitos vacía como prueba
cuando estos campos declaren una carencia. Con el detalle graph/analyzed,
revise también el `requirement_status` de cada objeto: solo `complete` hace que
una lista vacía demuestre cero requisitos para ese objeto. `partial` conserva
los hechos conocidos sin afirmar una cobertura exhaustiva; `unavailable`
significa que no se estableció ninguna cobertura utilizable. En ambos casos,
la evaluación de acoplamiento derivada de los requisitos de ese objeto sigue
siendo desconocida. `DBP1410W` identifica un catálogo de artefactos opcional
que no pudo leerse.

La topología anónima de dependencias aún puede identificar una aplicación. Apruebe `graph` o `analyzed` solo si ese riesgo es aceptable. Consulte [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Nivel 2: medición de compresión

El nivel 2 solo se habilita mediante el par explícito:

```bash
--measure-compression --yes
```

El nivel 2, además, lee muestras limitadas de filas en la memoria del proceso. Los bytes muestreados se codifican en un búfer en memoria y se utilizan para derivar mediciones de compresión agregada, densidad de nulos, cardinality/frequency, longitud y estilo, antes de que los valores y las huellas digitales temporales se descarten.

Los bytes de las muestras:

- no se escriben en `blueprint.toml`;
- no se escriben en el registro de auditoría;
- no se escriben en archivos temporales;
- no se envían por ninguna red aparte de la conexión de base de datos;
- no se conservan después de resumir la muestra.

El nivel 2 es valioso porque el tiempo de transferencia y el costo de salida dependen de los bytes comprimidos, no de los bytes de la tabla sin comprimir.

## Sonda de RTT

De forma predeterminada, la herramienta ejecuta cinco consultas `SELECT 1` después de establecer la conexión. Esto emite un bloque `[network]` que contiene `connect_total_ms`, `query_rtt_ms_p50` y `query_rtt_ms_p95`.

La sonda ayuda a comprender dónde se ejecutó la herramienta Blueprint con respecto a la base de datos de origen. No representa el RTT de la WAN de migración.

Deshabilítela con:

```bash
--no-rtt-probe
```

## Archivos leídos

Durante la ejecución, la herramienta solo lee archivos seleccionados
explícitamente en la línea de comandos o referenciados por un manifiesto por
lotes o paquete seleccionado explícitamente. Pueden ser archivos de contraseña,
usuario o clave de anonimización, archivos de CA/certificado/clave TLS,
archivos de tokens Entra, entradas de archivos estructurados y entradas
Blueprint o de paquetes.

Deliberadamente no lee ubicaciones implícitas habituales de credenciales como `~/.pgpass`, `~/.my.cnf`, archivos de credenciales de nube, claves SSH, el historial del shell ni variables de entorno de contraseñas predeterminadas.

Esa declaración cubre la detección de credenciales controlada por la
aplicación. Las bibliotecas de base de datos, TLS, DNS y autenticación integrada
pueden consultar almacenes de confianza, configuración y cachés de credenciales
del sistema operativo. Revise o rastree por separado esas dependencias de
plataforma cuando lo exija la política del host.

Consulte [`AUDIT.md`](AUDIT.md) para conocer la lista completa.

## Archivos escritos

La herramienta solo escribe en las rutas seleccionadas por el modo activo:

- el TOML Blueprint `--out` en modo en vivo;
- `--deck` si se solicita;
- `--audit-log` si se solicita;
- `--out-dir` en modo por lotes: `bundle.toml`, `blueprints/`, `audits/`, un
  marcador de propiedad y `errors.txt` cuando se debe informar de un fallo parcial;
- el registro de auditoría en stderr en cada ejecución.

No usa un directorio temporal implícito del sistema operativo. La publicación
atómica por lotes puede crear un directorio adyacente de preparación o recuperación
junto a `--out-dir`; un fallo gestionado lo elimina o restaura el paquete anterior.

## Lista de comprobación de la salida

Antes de compartir `blueprint.toml`, verifique que:

- la cabecera sea la cabecera fija `dbwarp-blueprint v7`;
- los identificadores de tabla tengan el aspecto `table-001`;
- los identificadores de columna tengan el aspecto `col-1`;
- los identificadores de esquema tengan el aspecto `schema-A`;
- no aparezcan nombres reales de tablas, columnas, índices, esquemas ni usuarios;
- no haya nombres de objetos no tabulares, texto de definiciones, cadenas de puntos de conexión, credenciales, material de claves/certificados, nombres de paquetes ni binarios;
- no aparezcan valores de filas;
- los valores numéricos utilicen la precisión exacta o redondeada documentada
  en [`FORMAT.md`](FORMAT.md); revise los campos exactos opcionales como datos
  más sensibles;
- las secciones opcionales derivadas de muestras contengan metadatos agregados
  de compresión, densidad de NULL, cardinalidad/frecuencia, longitud, estilo y
  procedencia de muestra, nunca valores muestreados.
- los campos de integridad de artefactos declaren la visibilidad filtrada, los catálogos ilegibles y las familias conocidas sin modelar.

La salida equilibrada predeterminada MySQL contiene las capacidades declaradas exactas y las longitudes de prefijo de índice, además de muestras promedio/p95 relativamente redondeadas. Revise los tres marcadores de fidelidad explícitamente. Si se utilizó `--length-fidelity exact --yes`, apruebe también las estadísticas muestreadas exactas. Los valores de las filas y los nombres reales de los objetos deben seguir estando ausentes. Un Blueprint sin marcadores de fidelidad fue generado por una versión anterior; recupérelo.

El indicador no afirma que el muestreo haya cubierto todas las tablas. Si se informa `DBP1406W`, aumente `--max-wall-secs` y vuelva a capturar.

## Seguridad operativa

Primera ejecución recomendada:

```bash
--sample-rows 500 --max-wall-secs 120
```

Ejecución similar a producción recomendada una vez aprobada:

```bash
--sample-rows 1000 --max-wall-secs 300
```

Ejecute desde una réplica de lectura si la política de producción prohíbe muestrear en la instancia principal.
