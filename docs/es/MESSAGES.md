# Códigos de mensajes para operadores

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../MESSAGES.md).

**Idiomas:** [English](../MESSAGES.md) | [Deutsch](../de/MESSAGES.md) | [Français](../fr/MESSAGES.md) | **Español** | [Polski](../pl/MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [中文](../zh/MESSAGES.md)

`dbwarp-blueprint` utiliza identificadores estables de mensajes para operadores en los errores de validación y flujo de trabajo que pertenecen a DBWarp.
El formato se inspira en los mensajes para operadores de estilo IBM: un prefijo de subsistema, un identificador numérico y un sufijo de gravedad.
La documentación de IBM CICS describe un identificador de programa junto con un número de mensaje de cuatro dígitos y una letra de gravedad; IBM MQ utiliza de forma similar campos de componente/prefijo, un identificador numérico y un código final de tipo de mensaje. Las directrices de Microsoft sobre mensajes de error refuerzan la regla práctica de que un error debería describir el problema y proporcionar una acción que el usuario pueda realizar.

Referencias:

- Formato de mensajes de IBM CICS: https://www.ibm.com/docs/en/cics-pa/5.3.0?topic=messages-message-format
- Disposición de la información de mensajes de IBM CICS: https://www.ibm.com/docs/en/cics-ts/6.x?topic=messages-format-cics-message-information
- Formato de mensajes de IBM MQ para z/OS: https://www.ibm.com/docs/SSFKSJ_9.2.0/com.ibm.mq.ref.doc/q050270_.htm
- Directrices de Microsoft sobre mensajes de error: https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error

## Formato

```text
DBPnnnnS message text. Next: corrective action.
```

Campos:

- `DBP` significa DBWarp Blueprint.
- `nnnn` es un número de mensaje estable de cuatro dígitos.
- `S` es la gravedad: `E` error, `W` advertencia, `I` información.

El código es estable y no depende del idioma. Su resumen, causa y acción
correctiva se localizan cuando `--lang` o la configuración regional del proceso
seleccionan un idioma admitido. Los detalles dinámicos del sistema operativo,
del controlador de la base de datos, de rutas y de la cadena causal permanecen
literales para que el personal de soporte pueda buscar el error original. El
texto del mensaje no debe incluir secretos ni URI de conexión sin ocultar.

## Intervalos

| Intervalo | Área |
|---|---|
| `DBP0001E` | Error encapsulado realmente sin clasificar, con cadena causal |
| `DBP10xxE` | Validación de comandos, entrada de conexión y política de recopilación |
| `DBP11xxE` | Validación del manifiesto por lotes y de la entrada de orígenes |
| `DBP12xxE` | Selectores de paquetes y selectores de URI Blueprint |
| `DBP13xxE` | Validación de TOML/presentaciones/esquemas sin conexión |
| `DBP14xxE/W` | Errores de captura de base de datos en vivo y degradación no fatal del muestreo |
| `DBP15xxE/W` | Archivos estructurados, Blueprints, presentaciones y salida de auditoría |
| `DBP16xxE/W` | Credenciales, autenticación, TLS y política de archivos sensibles |
| `DBP17xxE` | Consentimiento del operador |
| `DBP18xxE` | Inicialización del entorno de ejecución del proceso |

## Códigos actuales

| Código | Significado |
|---|---|
| `DBP0001E` | Error sin clasificar; a continuación aparece la cadena causal. |
| `DBP1000E` | Falta `--connect` fuera de los modos sin conexión. |
| `DBP1001E` | Se rechazó una contraseña incrustada en la URI. |
| `DBP1002E` | Esquema de URI de `--connect` no admitido. |
| `DBP1003E` | Sustitución del nombre de servidor TLS no admitida. |
| `DBP1004E` | Se utilizó una opción de token de Azure con un motor distinto de SQL Server. |
| `DBP1005E` | El modo de autenticación no está disponible para el motor seleccionado. |
| `DBP1006E` | Se solicitó muestreo de archivos estructurados sin `--yes` explícito. |
| `DBP1007E` | Se solicitó un modo explícito de fidelidad de longitudes para un motor que aún no expone ese contrato. |
| `DBP1008E` | El alias heredado de longitudes exactas entra en conflicto con la fidelidad de longitudes strict. |
| `DBP1009E` | Se solicitó fidelidad exacta de longitudes muestreadas sin `--yes` explícito. |
| `DBP1010E` | El catálogo de localización integrado está incompleto o es incoherente. |
| `DBP1011E` | Los argumentos de línea de comandos no son válidos. |
| `DBP1012E` | Una URI de conexión de base de datos admitida tiene un formato incorrecto. |
| `DBP1013E` | `--source-kind` está vacío o no se admite. |
| `DBP1014E` | Se solicitó un grafo de artefactos anónimo o un análisis de definiciones sin consentimiento explícito. |
| `DBP1015E` | Se utilizaron opciones de certificado TLS de cliente con SQL Server, cuyo controlador no las implementa. |
| `DBP1101E` | No se puede leer el manifiesto por lotes. |
| `DBP1102E` | No se puede analizar el manifiesto por lotes. |
| `DBP1103E` | El manifiesto por lotes no contiene entradas `[[source]]`. |
| `DBP1104E` | El modo por lotes necesita un `--yes` explícito. |
| `DBP1105E` | Falló un origen dentro de un lote. |
| `DBP1106E` | Tipo de origen por lotes no admitido. |
| `DBP1107E` | El origen de archivos no resolvió ningún archivo de entrada. |
| `DBP1108E` | Modo de conjunto de datos de archivos no admitido. |
| `DBP1109E` | El identificador del origen por lotes no contiene ninguna letra ni dígito ASCII utilizable. |
| `DBP1110E` | La fuente de base de datos tiene un número incorrecto de fuentes de conexión. |
| `DBP1111E` | La variable `connect_env` no existe o no se puede leer. |
| `DBP1112E` | `connect_file` no existe o no se puede leer. |
| `DBP1113E` | No se pudo completar la salida, auditoría, informe o directorio del lote. |
| `DBP1114E` | Los miembros del conjunto de datos de archivos estructurados son incompatibles. |
| `DBP1115E` | Fallaron todos los orígenes del lote; solo se publicó salida de diagnóstico. |
| `DBP1116E` | Se publicó un paquete de lote parcial. |
| `DBP1200E` | Sintaxis de selector o `blueprint://` no válida. |
| `DBP1201E` | El selector del paquete no coincidió con ningún origen. |
| `DBP1202E` | El selector del paquete coincidió con varios orígenes. |
| `DBP1203E` | El selector del paquete no coincidió con ningún Blueprint ni ninguna tabla extraíble. |
| `DBP1204E` | No se pudo leer la entrada del paquete. |
| `DBP1205E` | El contenido del paquete o del Blueprint al que hace referencia no es válido. |
| `DBP1206E` | No se pudo escribir la salida del paquete. |
| `DBP1301E` | A `--from-toml` le falta `--deck`. |
| `DBP1302E` | Versión del esquema TOML Blueprint no admitida. |
| `DBP1401E` | Falló el límite de captura de PostgreSQL. |
| `DBP1402E` | Falló el límite de captura de MySQL o MariaDB. |
| `DBP1403E` | Falló el límite de captura de SQL Server. |
| `DBP1404W` | El modo TLS `prefer` de PostgreSQL recurrió a texto sin cifrar en bucle local. |
| `DBP1405W` | La sonda opcional de RTT de la base de datos no estaba disponible. |
| `DBP1406W` | Se agotó el presupuesto de tiempo del muestreo de nivel 2. |
| `DBP1407W` | No estaba disponible una muestra de compresión. |
| `DBP1408W` | No estaba disponible una muestra de estilo de columna de texto. |
| `DBP1409W` | La tarea de conexión asíncrona de PostgreSQL notificó un error. |
| `DBP1410W` | Un catálogo de artefactos opcional no estaba disponible, por lo que se reduce explícitamente la integridad. |
| `DBP1411W` | La evidencia de topología no está disponible; el despliegue y el rol local siguen desconocidos. |
| `DBP1412W` | Se detectó un diseño distribuido o fragmentado, pero no había dimensionamiento agregado completo. |
| `DBP1413W` | La cobertura de tablas, filas o bytes es incompleta o desconocida. |
| `DBP1414W` | La relación de la fuente del paquete es desconocida; la aritmética entre fuentes no es segura. |
| `DBP1415W` | Las réplicas declaradas no coinciden; se conservó un representante determinista sin promediar. |
| `DBP1416W` | Un grupo de fragmentos está incompleto y no aporta totales agregados. |
| `DBP1417W` | Se suprimieron los totales agregados del paquete. |
| `DBP1418W` | Una fuente incluida en la aritmética del paquete tiene cobertura incompleta o desconocida. |
| `DBP1419E` | La captura en vivo superó `--max-wall-secs`; el cliente cerró la conexión e informa del límite de servidor específico del motor. |
| `DBP1420E` | Al menos un `--schema` solicitado no era visible, por lo que no se escribió ningún Blueprint con alcance ambiguo. |
| `DBP1421W` | Las identidades de sesión de SQL Server no estaban disponibles; la captura continuó sin afirmar una identidad. |
| `DBP1501E` | Falló el límite de captura de archivos estructurados. |
| `DBP1502E` | Falló la salida del Blueprint o el paquete. |
| `DBP1503E` | Falló la generación de la presentación de PowerPoint. |
| `DBP1504W` | No se pudo escribir el registro de auditoría. |
| `DBP1601E` | Falló la adquisición de credenciales. |
| `DBP1602E` | Falló la configuración TLS. |
| `DBP1603E` | Falló la adquisición del nombre de usuario de la base de datos. |
| `DBP1604E` | La configuración de autenticación de la base de datos no es válida. |
| `DBP1605W` | La aplicación de permisos de archivos sensibles no está disponible en esta plataforma. |
| `DBP1606E` | La aserción de la entidad de seguridad autenticada de SQL Server falló antes de capturar el catálogo. |
| `DBP1607E` | No se pudo inicializar de forma segura la clave HMAC de anonimización. |
| `DBP1701E` | La operación se canceló antes del consentimiento explícito. |
| `DBP1702E` | No se pudo leer la respuesta de consentimiento desde la entrada estándar. |
| `DBP1801E` | No se pudo inicializar el entorno de ejecución asíncrono. |

Cada idioma anunciado debe contener el resumen, la causa y la acción de todos
los códigos DBP actuales. El binario lo valida al iniciarse y falla con
`DBP1010E` en lugar de recurrir silenciosamente al inglés.

Los errores predecibles en los límites de decisión se ejercitan mediante una
matriz adversarial de la CLI. Una condición conocida debe emitir su código
específico como primer código para el operador y no debe recurrir a `DBP0001E`.
El componente que presenta el error también recorre toda la cadena de errores para que un contexto
de implementación sin código no pueda ocultar una causa interna codificada.

Las advertencias no fatales del muestreo de la base de datos se imprimen con su
código de advertencia estable y se registran en la auditoría de la ejecución.
Esto permite distinguir una captura de nivel 2 completa de una captura correcta
pero parcialmente muestreada sin convertir el fallo de una sonda opcional en un
fallo total de recopilación.

## Lista de comprobación para soporte

Cuando un cliente notifique un error, solicite:

- la salida completa de terminal, incluido el código `DBP`;
- el registro de auditoría si se utilizó `--audit-log`;
- la línea de comandos con los datos sensibles ocultos;
- para errores de paquetes, la salida de `dbwarp-blueprint --bundle-list ...`.

No solicite archivos de contraseñas, archivos de tokens, claves privadas ni muestras sin procesar de filas de la base de datos.
