# TLS y certificados

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../../TLS.md).

**Idiomas:** [English](../../TLS.md) | [Deutsch](../de/TLS.md) | [Français](../fr/TLS.md) | **Español** | [Polski](../pl/TLS.md) | [日本語](../ja/TLS.md) | [中文](../zh/TLS.md)

Utilice TLS siempre que la conexión con la base de datos atraviese un límite de red.
`verify-full` es el valor predeterminado: se validan la cadena del certificado y el nombre de host del servidor salvo que el operador seleccione otro modo.

## Opciones habituales

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

Configuración recomendada para producción:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## CA interna

Si el certificado de su base de datos está firmado por una CA interna:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## Discrepancia del nombre de host

Utilice un nombre de host de `--connect` que coincida con el certificado cuando
ejecute con `--tls-mode verify-full`. Esta versión no permite sustituir el nombre
del servidor TLS; el uso de `--tls-server-name` falla de forma explícita en vez
de debilitar silenciosamente la verificación. Si su política permite validar la
CA sin validar el nombre de host, utilice `--tls-mode verify-ca`.

Los valores predeterminados de confianza dependen del motor:

- PostgreSQL y MySQL utilizan los certificados raíz de Mozilla integrados en
  el binario cuando se omite `--tls-ca`. Un paquete PEM proporcionado sustituye
  esas raíces.
- SQL Server utiliza el almacén de confianza del sistema operativo cuando se
  omite `--tls-ca`. Un archivo `.pem` o `.crt` proporcionado debe contener
  exactamente un certificado de CA y sustituye las raíces del sistema
  operativo.

El controlador de SQL Server valida el nombre de host de la conexión tanto con
`verify-ca` como con `verify-full`; para este motor, `verify-ca` no es
deliberadamente menos estricto que `verify-full`.

## Modos de texto sin cifrar y compatibilidad

`prefer` solo se acepta para destinos de bucle local. PostgreSQL puede recurrir allí a texto sin cifrar local y emite `DBP1404W`; los demás motores siguen intentando TLS. En destinos remotos, `disable` y `require` necesitan `--i-know-what-im-doing`, pues permiten texto sin cifrar o cifran sin autenticar el servidor. Esa confirmación no hace que sean adecuados para producción.

## mTLS

PostgreSQL y MySQL admiten la autenticación mediante certificado de cliente. Si
alguna de estas bases de datos requiere un certificado de cliente:

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

En sistemas tipo Unix, los archivos de claves privadas no deben ser legibles por el grupo ni por todo el mundo.
La autenticación mediante certificado de cliente no está implementada para SQL
Server; proporcionar `--tls-cert`/`--tls-key` con ese motor falla con
`DBP1015E` en lugar de ignorar silenciosamente los archivos.

## Omitir la verificación

`--tls-skip-verify` está destinado exclusivamente al diagnóstico. No lo utilice para recopilar Blueprints de bases de datos de producción salvo que su equipo de seguridad lo haya aprobado explícitamente.

## Registro de auditoría

El registro de auditoría conserva el modo TLS solicitado, las rutas de CA y certificado y si se omitió la verificación. Tras una conexión correcta registra si se negoció TLS; como los controladores actuales no exponen una versión fiable, la marca como no disponible. No registra claves privadas.
