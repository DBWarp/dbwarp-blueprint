# Inventario de artefactos que no son tablas

> **Nota de traducción:** esta traducción asistida por máquina aún necesita revisión técnica nativa. La [versión canónica en inglés](../ARTIFACT_INVENTORY.md) prevalece y este texto no debe considerarse contractual.

**Idiomas:** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
[Français](../fr/ARTIFACT_INVENTORY.md) | **Español** |
[Polski](../pl/ARTIFACT_INVENTORY.md) | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Desde el esquema v4, los Blueprints describen objetos de base de datos que no son tablas y requisitos
de despliegue sin publicar sus nombres de origen, definiciones, cadenas de
puntos de conexión, secretos, certificados, claves ni binarios. El inventario ayuda a
DBWarp a estimar la complejidad de la migración y a identificar trabajo que
requiere paquetes, infraestructura, aprobación de seguridad o conversión
asistida.

El inventario no es una afirmación de capacidad. Que un objeto aparezca no
significa que DBWarp pueda recrearlo o traducirlo automáticamente. La capacidad
de migración se comprueba por separado en la matriz de rutas y artefactos de
DBWarp.

## Niveles de detalle

Use `--artifact-detail` para elegir el equilibrio entre privacidad y
planificación:

| Valor | Lecturas de base de datos | Salida Blueprint | Consentimiento |
|---|---|---|---|
| `none` | Sin catálogos ni definiciones de artefactos | Sin recuentos ni grafo | Sin consentimiento adicional |
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

- identificadores anónimos estables como `view-001`, `function-002` y `schema-A`;
- símbolos cerrados de clase, subclase, nivel, visibilidad y modo de seguridad;
- dependencias expresadas solo mediante identificadores anónimos de artefacto o tabla;
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
propietario las sobrescribe al liberarlas; no se serializan, registran ni envían
a otro servicio. Es una reducción de exposición en memoria, no una promesa
frente a paginación del sistema o un depurador privilegiado.

Incluso un grafo anónimo puede identificar una aplicación por sus recuentos y
topología. Por eso `graph` y `analyzed` fallan con `DBP1014E` sin `--yes`.

## Evidencia de completitud

El bloque `[artifact_inventory]` se audita a sí mismo:

| Campo | Significado |
|---|---|
| `contract` | Contrato con versión independiente; actualmente `dbwarp-blueprint-artifacts/v1` |
| `detail` | Nivel de detalle solicitado |
| `visibility` | `full`, `privilege_filtered` o `unknown` |
| `inventory_complete` | Verdadero solo con visibilidad total, sin catálogos ilegibles ni familias no modeladas declaradas |
| `dependencies_complete` | Verdadero solo si las fuentes de dependencias eran legibles y las familias modeladas están cubiertas |
| `analysis_complete` | Verdadero solo con `analyzed` y análisis completo de todas las definiciones disponibles |
| `catalogs_read` | Familias de catálogos estándar inspeccionadas correctamente |
| `catalogs_unreadable` | Familias que fallaron o no estaban disponibles |
| `families_not_inventoried` | Familias conocidas fuera del contrato actual |

Un fallo de catálogo opcional no elimina objetos en silencio. La ejecución emite
`DBP1410W`, registra el catálogo y fuerza a falso las afirmaciones de
completitud correspondientes. Una cuenta de pocos privilegios puede producir
un inventario parcial útil sin presentar ausencia como prueba.

## Cobertura por motor

El recopilador v1 modela estas familias:

| Motor | Familias de objetos modeladas |
|---|---|
| PostgreSQL | vistas, vistas materializadas, secuencias, rutinas, agregados, tipos enum/domain/composite/range, desencadenadores, valores predeterminados, comprobaciones, políticas, reglas, desencadenadores de eventos, extensiones, tablas/servidores externos, publicaciones, suscripciones, espacios de tablas y funciones nativas |
| MySQL | vistas, funciones y procedimientos almacenados, desencadenadores, eventos programados, dependencias de vistas, tablas FEDERATED y registros UDF cargables |
| SQL Server | vistas, procedimientos almacenados, funciones escalares/tabulares, módulos CLR, desencadenadores, valores predeterminados, comprobaciones, reglas, sinónimos, secuencias, tipos definidos por el usuario, ensamblados CLR, objetos de datos externos, catálogos de texto completo, objetos de partición, grupos de archivos no PRIMARY, certificados, claves, credenciales de base de datos, servidores vinculados y trabajos de SQL Server Agent |

Cada Blueprint enumera las familias conocidas no modeladas. Un recuento cero no
prueba ausencia salvo que `visibility`, los indicadores de completitud y la
lista de familias no inventariadas respalden esa conclusión.

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

El detalle `analyzed` añade bloques `dbwarp-language-feature-census/v1` para
definiciones SQL y procedurales disponibles. El primer analizador es
`lexical-v1` y declara `status = "partial"`; no es un parser, compilador,
enlazador semántico ni garantía de traducción.

Registra bandas limitadas de tamaño, sentencias, símbolos, anidación,
complejidad ciclomática y regiones opacas/dinámicas. Un vocabulario cerrado
describe control, uniones, subconsultas, CTE, agregados, ventanas, DML, DDL,
objetos temporales, SQL dinámico, JSON, XML, espacial, vector y seguridad. El
contexto incluye el perfil gramatical normalizado, modos SQL de MySQL y, para
SQL Server, compatibilidad, `ANSI_NULLS` y `QUOTED_IDENTIFIER`.

El analizador elimina comentarios, literales e identificadores entre comillas.
Reglas de contexto cubren eventos de desencadenador, `EXECUTE FUNCTION` de
PostgreSQL y opciones de módulos SQL Server. Los resultados siguen siendo
evidencia aproximada. Un futuro analizador gramatical podrá usar otra versión
sin cambiar el contrato exterior.

## Flujo de revisión recomendado

1. Ejecute `summary` con la revisión normal de catálogos.
2. Revise recuentos, clases externas, visibilidad, catálogos ilegibles y familias no modeladas.
3. Apruebe `graph` solo si acepta la topología anónima.
4. Apruebe `analyzed` solo si acepta la lectura transitoria de definiciones.
5. Conserve el registro de auditoría localmente como evidencia con acceso controlado. Compártalo solo cuando un destinatario identificado necesite los detalles de puntos de conexión, identidades, rutas y degradaciones a través de un canal seguro aprobado.
6. Compare el inventario con la matriz de capacidades DBWarp antes de prometer recreación o traducción automática.

Los campos exactos están en la [Referencia de formato](FORMAT.md). Las lecturas,
escrituras, advertencias y afirmaciones de confianza están en la [Referencia de
auditoría](AUDIT.md).
