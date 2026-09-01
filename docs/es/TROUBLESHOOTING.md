# Resolución de problemas

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../TROUBLESHOOTING.md).

**Idiomas:** [English](../TROUBLESHOOTING.md) | [Deutsch](../de/TROUBLESHOOTING.md) | [Français](../fr/TROUBLESHOOTING.md) | **Español** | [Polski](../pl/TROUBLESHOOTING.md) | [日本語](../ja/TROUBLESHOOTING.md) | [中文](../zh/TROUBLESHOOTING.md)

Errores habituales de `dbwarp-blueprint` y pasos que debe seguir.

Los errores que puede resolver el operador comienzan ahora con un código de mensaje estable `DBPnnnnS`, por ejemplo `DBP1001E`.
Utilice el código para buscar en la documentación o abrir un ticket de soporte. Consulte [Códigos de mensajes para operadores](MESSAGES.md).

## No se utiliza el idioma solicitado

Utilice un valor admitido explícito al diagnosticar la selección de la configuración regional:

```bash
dbwarp-blueprint --lang pl --help
```

Los valores admitidos son `en`, `de`, `fr`, `es`, `pl`, `ja` y `zh`. Sin
`--lang`, la herramienta comprueba `DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES`
y `LANG`, en ese orden. Un valor explícito no admitido se rechaza con
`DBP1011E`; un catálogo integrado incompleto provoca un error de inicio con
`DBP1010E` en lugar de recurrir al inglés.

En Windows las variables de configuración regional suelen estar ausentes; pase `--lang` o defina `DBWARP_BLUEPRINT_LANG`.

## La anchura o los colores del banner son incorrectos

La anchura procede de `COLUMNS` cuando está definida; en otro caso, de la consola en Linux y macOS, o de 80 columnas. La capacidad de color procede de `NO_COLOR`, `TERM` y `COLORTERM`; si falta `TERM`, algo normal en Windows, se usan 16 colores. Use `--color always`, `--color never` o defina `COLUMNS` para anularlo.

## Se rechaza la contraseña en la URI

Síntoma:

```text
DBP1001E refusing to use URI-embedded password
```

Solución: elimine la contraseña de la URI y utilice una de estas opciones:

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

En Unix, el modo del archivo no debe permitir la lectura por parte del grupo ni de otros usuarios.

## Error de permisos del archivo de contraseña

Síntoma: la herramienta rechaza `--password-file` o `--tls-key` porque los permisos son demasiado amplios.

Solución:

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

Esto evita la divulgación accidental a través de otros usuarios locales del mismo host.

## Falla la verificación TLS

Utilice `--tls-mode verify-full` con el paquete de CA y el nombre de host correctos:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

Si el nombre de host del certificado no coincide, corrija el nombre DNS o el certificado. `--tls-skip-verify` se rechaza en hosts que no sean de bucle invertido salvo que también se proporcione `--i-know-what-im-doing`; no lo utilice en producción.

## Raíces de confianza TLS de SQL Server

En SQL Server, los modos que verifican certificados utilizan el almacén de
confianza del sistema operativo cuando se omite `--tls-ca`. Un archivo `.pem` o
`.crt` proporcionado debe contener exactamente un certificado de CA y sustituye
esas raíces. El controlador comprueba el nombre de host de la conexión tanto con
`verify-ca` como con `verify-full`.

## El nivel 2 requiere consentimiento

Síntoma:

```text
--measure-compression requires --yes
```

Solución:

```bash
--measure-compression --yes
```

Esto es deliberadamente explícito porque el nivel 2 lee muestras acotadas de filas en memoria antes de descartarlas.

## El muestreo tarda demasiado

Reduzca uno o ambos valores:

```bash
--sample-rows 500
--max-wall-secs 120
```

Para la primera revisión de producción, es mejor una muestra de nivel 2 más pequeña que no realizar ninguna medición de compresión. Si los resultados están sesgados o incompletos, vuelva a ejecutar desde una réplica con un presupuesto mayor.

## El personal de administración de bases de datos prohíbe la sonda SELECT 1 que no es de catálogo

Deshabilite la sonda de RTT:

```bash
--no-rtt-probe
```

La sonda de RTT predeterminada consta de cinco consultas `SELECT 1` y no lee datos de filas, pero algunas políticas clasifican cualquier consulta que no sea de catálogo como fuera de alcance.

## La salida no contiene secciones de compresión

Las secciones de compresión solo aparecen cuando están presentes ambas opciones:

```bash
--measure-compression --yes
```

Los Blueprints de solo catálogo son válidos, pero las estimaciones posteriores de compresión serán inferidas.

## Algunas muestras de compresión aparecen marcadas como sesgadas

Algunos motores no ofrecen un muestreo uniforme de tablas en todos los casos, y las tablas pequeñas pueden requerir una alternativa con `LIMIT`. El archivo Blueprint registra `sampled_with_bias` y `bias_reason` para que el estimador y la persona responsable de la revisión puedan tenerlo en cuenta.

Las muestras sesgadas siguen siendo útiles; simplemente no ofrecen garantías tan sólidas como las muestras uniformes.

## Falla la generación de una presentación desde TOML

`--from-toml` debe utilizarse junto con `--deck`:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

No incluya opciones de base de datos en vivo con `--from-toml`. La herramienta rechaza modos combinados en vivo/sin conexión para mantener sencillo el límite de auditoría.

## El archivo Blueprint parece demasiado pequeño

Un archivo Blueprint normal es compacto. Contiene metadatos estructurales, recuentos redondeados, índices, la estructura del grafo de claves foráneas y resúmenes opcionales de compresión. No debe contener valores de filas ni identificadores.

Si necesita una base de datos representativa para pruebas de rendimiento, entregue el archivo `blueprint.toml` aprobado a las herramientas posteriores, revisadas por separado y autorizadas para ese trabajo.

## Necesidad de demostrar que no se realizó ninguna carga

Utilice el registro de auditoría y herramientas de red:

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

El comportamiento de red esperado durante la ejecución depende del modo activo.
Una ejecución en vivo con `--connect` abre la sesión de base de datos solicitada;
DNS puede ponerse en contacto con el resolutor configurado, y la autenticación
integrada Kerberos/SSPI puede contactar con un KDC o controlador de dominio. El
modo por lotes abre una sesión de base de datos por cada origen de base de datos.
Las operaciones locales de TOML, Parquet, Avro y paquetes no inician ninguna
conexión de red de la aplicación, aunque las rutas montadas en red siguen
estando sujetas a la pila de almacenamiento del host.
