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
- sin privilegios de DDL;
- sin rol de superusuario o administrador;
- acceso de lectura limitado a la base de datos que se evalúa;
- contraseña o token suministrado mediante archivo o solicitud interactiva, no incrustado en la URI.

Los permisos exactos varían según el motor y la política del cliente. Si la cuenta no puede leer algunas vistas de catálogo o muestrear algunas tablas, la herramienta debería fallar de forma clara o emitir un Blueprint reducido; conserve el registro de auditoría.

Utilice los scripts que tienen en cuenta la versión y las salvedades de
[`../../sql/grants/README.md`](../../sql/grants/README.md). Después de la captura
aprobada, elimine la cuenta dedicada del recopilador con el script
correspondiente de `sql/revoke/`; revise la base de datos, el patrón de host, el
rol y los destinos de inicio de sesión exactos antes de ejecutarlo.

## Nivel 1: solo catálogo

El nivel 1 es el valor predeterminado cuando no se utiliza `--measure-compression`.

Lee:

- la versión del motor;
- la lista de tablas y las entradas de ordenación anonimizadas;
- recuentos aproximados de filas;
- tamaños de tablas e índices;
- familias de tipos de columnas, posibilidad de valores nulos y estadísticas de longitud redondeadas cuando están disponibles;
- tipo de índice, unicidad y ordinales de columnas anonimizados;
- estructura del grafo de claves foráneas cuando está disponible;
- sonda opcional de RTT desde el entorno del cliente, salvo que se establezca `--no-rtt-probe`.

No lee valores de filas.

## Inventario de artefactos no tabulares

Desde el esquema v4, los Blueprints inventarían objetos no tabulares de forma independiente del muestreo de filas. De forma predeterminada, `--artifact-detail summary` lee catálogos de objetos, pero no definiciones, y solo emite recuentos acotados y clases de requisitos externos.

`--artifact-detail graph --yes` añade identificadores de objeto anónimos y aristas de dependencia. `--artifact-detail analyzed --yes` también lee temporalmente las definiciones disponibles y solo emite bandas léxicas acotadas de características y complejidad. Nunca se serializan texto de definiciones, nombres de objetos de origen, puntos de conexión, cadenas de proveedor, entidades de seguridad, secretos, claves, certificados, nombres de paquetes ni binarios.

Los privilegios de catálogo afectan a las afirmaciones de ausencia. Revise `visibility`, `inventory_complete`, `dependencies_complete`, `catalogs_unreadable` y `families_not_inventoried`; no interprete un recuento cero como prueba cuando estos campos declaren una carencia. `DBP1410W` identifica un catálogo de artefactos opcional que no pudo leerse.

La topología anónima de dependencias aún puede identificar una aplicación. Apruebe `graph` o `analyzed` solo si ese riesgo es aceptable. Consulte [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Nivel 2: medición de compresión

El nivel 2 solo se habilita mediante el par explícito:

```bash
--measure-compression --yes
```

Además, el nivel 2 lee muestras acotadas de filas en la memoria del proceso. Los bytes muestreados se codifican en un búfer interno de tramas de filas, se comprimen localmente con zstd en el nivel 3, se resumen en forma de proporciones redondeadas y se descartan.

Los bytes de las muestras:

- no se escriben en `blueprint.toml`;
- no se escriben en el registro de auditoría;
- no se escriben en archivos temporales;
- no se envían por ninguna red aparte de la conexión de base de datos;
- no se conservan después de resumir la muestra.

El nivel 2 resulta valioso porque el rendimiento de DBWarp y el coste del tráfico saliente dependen de los bytes comprimidos, no de los bytes sin procesar de la tabla.

## Sonda de RTT

De forma predeterminada, la herramienta ejecuta cinco consultas `SELECT 1` después de establecer la conexión. Esto emite un bloque `[network]` que contiene `connect_total_ms`, `query_rtt_ms_p50` y `query_rtt_ms_p95`.

La sonda ayuda a comprender dónde se ejecutó la herramienta Blueprint con respecto a la base de datos de origen. No representa el RTT de la WAN de migración.

Deshabilítela con:

```bash
--no-rtt-probe
```

## Archivos leídos

Durante la ejecución, la herramienta solo lee archivos indicados explícitamente en la línea de comandos, como archivos de contraseña y usuario, archivos de CA/certificado/clave TLS, archivos de tokens Entra o un archivo de entrada `--from-toml`.

Deliberadamente no lee ubicaciones implícitas habituales de credenciales como `~/.pgpass`, `~/.my.cnf`, archivos de credenciales de nube, claves SSH, el historial del shell ni variables de entorno de contraseñas predeterminadas.

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

- la cabecera sea la cabecera fija `dbwarp-blueprint v6`;
- los identificadores de tabla tengan el aspecto `table-001`;
- los identificadores de columna tengan el aspecto `col-1`;
- los identificadores de esquema tengan el aspecto `schema-A`;
- no aparezcan nombres reales de tablas, columnas, índices, esquemas ni usuarios;
- no haya nombres de objetos no tabulares, texto de definiciones, cadenas de puntos de conexión, credenciales, material de claves/certificados, nombres de paquetes ni binarios;
- no aparezcan valores de filas;
- los valores numéricos estén redondeados como se documenta en [`FORMAT.md`](FORMAT.md);
- las secciones opcionales de compresión contengan únicamente proporciones y metadatos de muestras.
- los campos de integridad de artefactos declaren la visibilidad filtrada, los catálogos ilegibles y las familias conocidas sin modelar.

La salida MySQL balanced predeterminada contiene capacidades declaradas y
longitudes de prefijos de índice exactas, además de muestras media/p95 con
redondeo relativo. Revise expresamente los tres marcadores de fidelidad. Si se
utilizó `--length-fidelity exact --yes`, apruebe también las estadísticas exactas
muestreadas. Los valores de filas y los nombres reales de objetos deben seguir
ausentes. Los marcadores de fidelidad que falten son heredados o desconocidos y
no deben tratarse como metadatos aptos para pruebas de rendimiento.

El marcador no afirma que el muestreo haya abarcado todas las tablas. Una
entrega para pruebas de rendimiento también debe mostrar en el manifiesto del
estimador cero columnas indexadas, de anchura variable y sin muestrear; aumente
`--max-wall-secs` y vuelva a capturar si no se supera este control.

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
