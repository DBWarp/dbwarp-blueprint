# Inventario de artefactos que no son tablas

> **Nota de traducción:** esta traducción asistida por máquina aún necesita revisión técnica nativa. La [versión canónica en inglés](../ARTIFACT_INVENTORY.md) prevalece y este texto no debe considerarse contractual.

**Idiomas:** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
[Français](../fr/ARTIFACT_INVENTORY.md) | **Español** |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Los Blueprints pueden describir objetos de base de datos que no son tablas y los requisitos previos de implementación sin publicar sus nombres de origen, definiciones, cadenas de punto final, secretos, certificados, claves o binarios. Este inventario ayuda a DBWarp a estimar la complejidad de la migración e identificar el trabajo que necesita paquetes, infraestructura, aprobación de seguridad o conversión asistida.

El inventario no es una declaración de capacidad. El hecho de que se informe sobre un objeto no significa que DBWarp pueda recrearlo o traducirlo automáticamente. Confirme con DBWarp qué tipos de objetos son compatibles.

## Niveles de detalle

Use `--artifact-detail` para elegir el equilibrio entre privacidad y
planificación:

| Valor | Lecturas de base de datos | Salida Blueprint | Consentimiento |
|---|---|---|---|
| `none` | Sin catálogos de inventario ni definiciones (la sonda de topología que solo cuenta sigue ejecutándose) | Inventario v7 explícitamente no solicitado; sin recuentos ni grafo | Sin consentimiento adicional |
| `summary` | Catálogos, pero no definiciones | Recuentos por clase de objeto y requisito externo | Predeterminado; sin consentimiento adicional |
| `graph` | Catálogos y metadatos de dependencias, pero no definiciones | Recuentos, objetos anónimos estables y aristas | Requiere `--yes` |
| `analyzed` | Catálogos, dependencias y definiciones disponibles | Grafo y bandas limitadas de lenguaje y complejidad | Requiere `--yes` |

El valor predeterminado es `summary`. Use `none` si la política permite capturar la
estructura de las tablas pero prohíbe los catálogos que no son tablas. Use `graph` para
planificar dependencias sin leer definiciones y `analyzed` solo tras aprobar la
lectura transitoria de definiciones.

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## Contrato de privacidad

La salida de artefactos contiene únicamente metadatos limitados y de
vocabulario cerrado:

- identificadores anónimos coherentes dentro de una ejecución, como
  `view-001`, `function-002` y `schema-A`; la estabilidad entre ejecuciones
  requiere reutilizar el mismo archivo protegido `--anonymization-key-file`;
- símbolos cerrados de clase, subclase, nivel, visibilidad y modo de seguridad;
- relaciones tipadas expresadas solo mediante identificadores anónimos de artefacto o tabla, con tokens cerrados de evidencia y motivo no resuelto;
- requisitos de funciones acotados y calificados por motor para planificar la migración;
- recuentos y bandas limitadas, no descripciones libres;
- etiquetas de catálogo estándar como `pg_proc`, `information_schema.views` o `sys.objects`;
- clases de requisitos externos, nunca sus nombres ni su material.

No contiene nombres de objetos de origen, texto SQL o procedural, nombres de
esquemas, entidades de seguridad, cadenas de puntos de conexión, cadenas de
proveedor, credenciales, claves, cuerpos de certificados, archivos de
ensamblado, nombres de paquetes de extensión ni nombres de bibliotecas
cargables.

En modo `analyzed`, las definiciones permanecen solo el tiempo necesario para
eliminar comentarios y literales y obtener agregados léxicos limitados. Un
propietario las sobrescribe al liberarlas; no se serializan, no se escriben en
los registros ni en el registro de auditoría, ni se envían a otro servicio. Es
una reducción de exposición en memoria, no una promesa
frente a paginación del sistema o un depurador privilegiado.

Incluso un grafo anónimo puede identificar una aplicación por sus recuentos y
topología. Por eso `graph` y `analyzed` fallan con `DBP1014E` sin `--yes`.

## Evidencia de completitud

El bloque `[artifact_inventory]` se audita a sí mismo:

| Campo | Significado |
|---|---|
| `contract` | Contrato con versión independiente; v7 usa `dbwarp-blueprint-artifacts/v2` y los esquemas Blueprint anteriores conservan v1 |
| `detail` | Nivel de detalle solicitado |
| `scope` | Alcance del catálogo v7: `all-visible-schemas`, `selected-schemas`, `structured-source` o `unknown` |
| `visibility` | `full`, `privilege_filtered` o `unknown` |
| `inventory_complete` | Verdadero solo con visibilidad total, sin catálogos ilegibles ni familias no modeladas declaradas |
| `dependencies_complete` | Verdadero solo si las fuentes de dependencias eran legibles y las familias modeladas están cubiertas |
| `requirements_complete` | Agregado V7: verdadero solo después de comprobar la versión y edición del motor, con cobertura completa de la población de evaluación para el alcance seleccionado y `requirement_status = complete | not_applicable` en cada artefacto emitido; la omisión significa falso |
| `analysis_complete` | Verdadero solo con `analyzed` y análisis completo de todas las definiciones disponibles |
| `catalogs_read` | Familias de catálogos estándar inspeccionadas correctamente |
| `catalogs_unreadable` | Familias que fallaron o no estaban disponibles; se degradan las afirmaciones afectadas sin borrar evidencia independiente por objeto |
| `catalogs_not_applicable` | Familias cuya inaplicabilidad se ha demostrado; disjuntas de los conjuntos legible e ilegible |
| `families_not_inventoried` | Familias de objetos conocidas omitidas del inventario de esta versión |

Un fallo de catálogo opcional no elimina objetos en silencio. La ejecución emite
`DBP1410W`, registra el catálogo y fuerza a falso las afirmaciones de
completitud correspondientes. Una cuenta de pocos privilegios puede producir
un inventario parcial útil sin presentar ausencia como prueba.

`object_count` cuenta registros de artefactos emitidos, no filas de un único
catálogo nativo. Los paquetes y tipos de objeto Oracle se modelan como
especificación, cuerpo y miembros; el cuerpo posee el análisis de lenguaje
combinado. Un objeto visto en varios catálogos se cuenta una vez. Los metadatos
y el código de un desencadenador se unen por identidad nativa antes de anonimizar.

## Contrato de complejidad agregada

El esquema v7 define `[artifact_inventory.complexity]`, un registro solo
agregado para `graph` y `analyzed`. No añade lecturas ni permisos: la evaluación
se deriva del grafo anónimo y el censo de lenguaje ya aprobados. Es obligatorio
para `graph` y `analyzed`, y está ausente en `none` y `summary`.

El informe de evaluación describe siete dimensiones: volumen, flujo de control, amplitud de funciones, interconexión, acoplamiento ambiental, opacidad y acoplamiento de dialectos. Los resultados se presentan en rangos, no como una puntuación numérica. `overall_score` está reservado y no se completa, porque un valor de 0 a 100 implicaría una precisión no soportada.

Cada dimensión contiene un histograma exacto unidimensional de la población
elegible. El volumen usa bandas de tamaño y las otras seis bandas de recuento.
Ambas formas añaden `not_applicable` y `unknown`, y cumplen
`eligible = assessed + not_applicable + unknown`. No hay compuesto por objeto ni
tablas cruzadas por tipo, función o esquema. Los objetos generados por el motor
y secundarios identificados de forma positiva se excluyen de la evaluación,
pero siguen visibles; los temporales y los que carecen de indicadores siguen
siendo elegibles. Los complementos instalados en el sitio, ensamblados CLR, Java y bibliotecas
siguen siendo trabajo real aunque no se pueda leer su cuerpo; solo un indicador
afirmativo de generación por el motor los excluye.

`assessment_population_complete` indica si se conocen todos los objetos
elegibles. Es independiente de `inventory_complete` y su omisión significa
falso. Una población incompleta fuerza una banda global `unknown`, salvo que el
límite inferior conocido ya sea `very-high`.

La cobertura se registra por dimensión. `not_applicable` es una evaluación
completa. Parcial significa que se conoce una dimensión aplicable y otra no;
sin evaluar significa que no se conoce ninguna aplicable. La evidencia
desconocida nunca equivale a baja complejidad. Una dimensión parcial es
`unknown`, salvo que su límite inferior ya sea `very-high`; la banda global solo
se emite cuando coinciden los límites. `graph` no emite veredicto global para
una población no vacía porque no lee definiciones. Una población vacía y
completa es `not-applicable`.

Los objetos encapsulados contribuyen al histograma de opacidad al "bucket" `unknown`, incluso cuando no se puede producir un censo parcial del lenguaje. Lea la banda de opacidad junto con su cobertura para que una banda observada pequeña no se lea sin su población desconocida. Si la evaluación en sí falla, el inventario completo se conserva con un agregado canónico de "todo desconocido". Las definiciones encapsuladas o retenidas, los gráficos incompletos, la evidencia de requisitos incompleta, los límites de alcance seleccionados y los lenguajes o dialectos no admitidos siguen siendo limitaciones explícitas. Las aserciones de limitación se derivan del artefacto y la evidencia del censo siempre que sea posible. `unsupported-dialect` sigue siendo distinto porque el censo puede nombrar un dialecto e informar `unavailable`, pero no tiene estado `unsupported`; significa que la definición se leyó, pero el analizador nombrado no admite ese dialecto, no que la fuente se haya retenido o encapsulado.

El registro contiene la única versión del analizador y conjuntos ordenados de intervalos de análisis, dialectos y perfiles de gramática presentes en el censo elegible. Dos capturas solo son comparables cuando esos conjuntos, el contrato, el evaluador, el alcance y la política de población coinciden. Un paquete conserva la complejidad por cada fuente secundaria y nunca la agrega entre motores o analizadores.

La versión del contrato de complejidad y del evaluador son independientes. Para obtener información precisa sobre los campos y las invariantes, consulte la [Referencia de formato](FORMAT.md).

## Cobertura por motor

El recopilador actual modela estas familias:

| Motor | Familias de objetos modeladas |
|---|---|
| PostgreSQL | vistas, vistas materializadas, secuencias, rutinas, agregados, tipos enum/domain/composite/range, desencadenadores, valores predeterminados, comprobaciones, políticas, reglas, desencadenadores de eventos, extensiones, tablas/servidores externos, publicaciones, suscripciones, espacios de tablas y funciones nativas |
| MySQL | vistas, funciones y procedimientos almacenados, desencadenadores, eventos programados, dependencias de vistas, tablas FEDERATED y registros UDF cargables |
| SQL Server | vistas, procedimientos almacenados, funciones escalares/tabulares, módulos CLR, desencadenadores, valores predeterminados, comprobaciones, reglas, sinónimos, secuencias, tipos definidos por el usuario, ensamblados CLR, objetos de datos externos, catálogos de texto completo, objetos de partición, grupos de archivos no PRIMARY, certificados, claves, credenciales de base de datos, servidores vinculados y trabajos de SQL Server Agent |

Cada Blueprint enumera las familias conocidas no modeladas. Un recuento cero no
prueba ausencia salvo que `visibility`, los indicadores de completitud y la
lista de familias no inventariadas respalden esa conclusión.

## Evidencia de requisitos

Los requisitos de los artefactos son datos del motor provenientes de columnas de catálogo delimitadas o de comprobaciones de sintaxis específicas del motor. El análisis léxico genérico no crea requisitos específicos del motor. Cuando una característica del lenguaje analizada refleja el mismo dato, el requisito tiene prioridad y la característica permanece como una observación léxica. La falta de un requisito no es prueba de que se haya comprobado cada token de requisito.

Cada objeto de la versión v7 graph/analyzed registra `requirement_status` como `complete`, `partial`, `unavailable` o `not_applicable`. Solo `complete` convierte una lista vacía en una prueba de cero requisitos para ese objeto. `partial` registra que algún hecho o productor tuvo éxito sin una cobertura exhaustiva; `unavailable` registra que ningún productor estableció una cobertura utilizable y, por lo tanto, no puede acompañar a la evidencia conocida de requisitos o prerrequisitos externos. Tal evidencia requiere `partial`. Ambos contribuyen a observaciones de complejidad derivadas de requisitos desconocidos, mientras que los objetos completos y sin afectar siguen siendo evaluables. `not_applicable` prohíbe los registros de requisitos y prerrequisitos externos. El valor `requirements_complete` a nivel de inventario solo es verdadero después de las comprobaciones de la versión y edición del motor, una población de evaluación completa y cuando cada objeto emitido está completo o no es aplicable. PostgreSQL, MySQL y SQL Server establecen el agregado solo después de que se haya intentado cada catálogo de artefactos aplicable y se haya demostrado que la población del alcance seleccionado está completa. Un catálogo denegado o ilegible, o un límite de selección cuya población no se puede demostrar, mantiene el agregado en falso sin borrar la evidencia completa por objeto de los catálogos que se leyeron. Los requisitos de los artefactos no se informan para Oracle.

## Requisitos externos

Los objetos que dependen de algo más que DDL de tabla portátil reciben una
clase anónima de requisito externo:

| Clase | Lo que debe resolver el operador |
|---|---|
| `postgresql_extension` | Paquete de extensión compatible y versión de destino |
| `postgresql_native_function` | Biblioteca nativa y compatibilidad ABI |
| `mysql_loadable_udf` | Binario UDF cargable y supuestos ABI del servidor de origen |
| `sqlserver_clr_assembly` | Habilitación CLR, ensamblado, runtime y política de confianza |
| `foreign_endpoint` | Red, proveedor, base de datos remota y autenticación |
| `replication_topology` | Topología de publicación/suscripción y política de destino |
| `physical_storage` | Diseño de grupos de archivos o ubicación física |
| `server_feature` | Disponibilidad de característica del servidor o servicio gestionado |
| `certificate_material` | Emisión o importación de certificado conforme a política |
| `encryption_or_credential_material` | Claves, credenciales, almacén externo y gestión de secretos |
| `sqlserver_agent` | Disponibilidad del agente, entorno y gobierno de trabajos |

El Blueprint indica si hace falta material binario, secreto o de punto de
conexión, pero no lo captura. Los objetos externos deben convertirse en tareas explícitas de
migración, no en omisiones de mejor esfuerzo.

## Censo de características del lenguaje

`analyzed` El detalle agrega `dbwarp-language-feature-census/v1` bloques. El esquema v7 emite `lexical-v2`, que analiza solo el cuerpo ejecutable o declarativo y registra `analysis_span = "executable-body"`. Excluye el envoltorio de creación externo, la identidad, la firma, la declaración de retorno y las opciones del módulo. Si el cuerpo no se puede aislar de forma segura, el recolector registra un rango desconocido y evidencia no disponible; los objetos sin dimensión de definición utilizan `not-applicable`. Un rango omitido se interpreta como desconocido en lugar de inferirse del motor. El analizador informa `status = "partial"` para definiciones admitidas porque no es un analizador, compilador, vinculador semántico ni una garantía de éxito de la traducción. La evidencia faltante o no admitida de la definición es `unavailable`, mientras que un análisis demostrado como inaplicable es `not_applicable`.

Registra bandas limitadas de tamaño, sentencias, símbolos, anidación,
complejidad ciclomática y regiones opacas/dinámicas. Un vocabulario cerrado
describe control, joins, subconsultas, CTE, agregados, ventanas, DML, DDL,
objetos temporales, SQL dinámico, JSON, XML, espacial, vector, errores lanzados,
control de transacciones, ref cursors, tipos anclados, intervalos, zonas horarias,
Boolean, LOB y seguridad. El
contexto incluye el perfil gramatical normalizado, modos SQL de MySQL y, para
SQL Server, compatibilidad, `ANSI_NULLS` y `QUOTED_IDENTIFIER`.

El analizador léxico elimina comentarios, literales entre comillas e identificadores entre comillas antes de contar. Tiene reglas de contexto para las declaraciones de eventos de activación, PostgreSQL `EXECUTE FUNCTION` y las opciones de los módulos de SQL Server. A pesar de esto, todos los resultados siguen siendo evidencia de planificación general. El elemento envuelto PL/SQL es rechazado; los bytes ofuscados nunca se convierten en medidas corporales plausibles.

## Flujo de revisión recomendado

1. Ejecute el nivel predeterminado `summary` con una revisión de los catálogos
   de artefactos. Si la política solo permite catálogos de tablas, use en su
   lugar `--artifact-detail none`; v7 registra explícitamente la decisión en vez
   de omitir el estado del inventario.
2. Revise recuentos, clases externas, visibilidad, catálogos ilegibles y familias no modeladas.
3. Apruebe `graph` solo si acepta la topología anónima.
4. Apruebe `analyzed` solo si acepta la lectura transitoria de definiciones.
5. Conserve el registro de auditoría localmente como evidencia con acceso controlado. Compártalo solo cuando un destinatario identificado necesite los detalles de puntos de conexión, identidades, rutas y degradaciones a través de un canal seguro aprobado.
6. No suponga que un objeto inventariado puede recrearse o traducirse automáticamente; confírmelo con DBWarp.

Para conocer los campos serializados exactos, consulte la [referencia del formato](FORMAT.md). Para las lecturas, escrituras, advertencias y afirmaciones de confianza durante la ejecución, consulte la [referencia de auditoría](AUDIT.md).
