# Autenticación

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../../AUTH.md).

**Idiomas:** [English](../../AUTH.md) | [Deutsch](../de/AUTH.md) | [Français](../fr/AUTH.md) | **Español** | [Polski](../pl/AUTH.md) | [日本語](../ja/AUTH.md) | [中文](../zh/AUTH.md)

`dbwarp-blueprint` admite los modos de autenticación necesarios con mayor frecuencia para recopilar Blueprints de PostgreSQL, MySQL y SQL Server.

## Nombre de usuario

Puede proporcionar el nombre de usuario en la URI o por separado:

```bash
--connect postgresql://app@db.internal/payments
```

o bien:

```bash
--connect postgresql://db.internal/payments --user app
```

Para nombres de usuario difíciles de codificar en una URI, utilice:

```bash
--user-file /path/to/user.txt
--user-env DB_USER
```

## Contraseña

Opción recomendada:

```bash
--password-file /path/to/password.txt
```

Alternativa:

```bash
--password-env DB_PASSWORD
```

Si no se proporciona ninguna fuente de contraseña, la herramienta la solicita de forma interactiva cuando es posible.

Se rechazan las contraseñas incrustadas en la URI de conexión.

## Token de SQL Server Entra ID

Para Azure SQL Database o Managed Instance con Microsoft Entra ID, genere el token con sus herramientas habituales y entréguelo a `dbwarp-blueprint` como secreto.

Archivo de token:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-file /secure/path/token.txt \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out blueprint.toml
```

Variable de entorno indicada por nombre:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-env AZURE_SQL_TOKEN \
  --tls-mode verify-full \
  --out blueprint.toml
```

La herramienta no llama a Azure CLI, no renueva tokens ni escribe el token en disco.

## Autenticación integrada de SQL Server

La autenticación integrada utiliza las credenciales del sistema operativo que ya estén presentes en el host.

Kerberos/GSSAPI en Linux:

```bash
kinit user@EXAMPLE.COM
DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh
./target/release/dbwarp-blueprint \
  --connect sqlserver://db.internal,1433/payments \
  --auth-mode integrated \
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' \
  --tls-mode verify-full \
  --out blueprint.toml
```

SSPI en Windows:

```powershell
.\dbwarp-blueprint.exe `
  --connect sqlserver://db.internal,1433/payments `
  --auth-mode integrated `
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' `
  --tls-mode verify-full `
  --out blueprint.toml
```

En el modo integrado, `dbwarp-blueprint` no lee ninguna contraseña. El sistema operativo suministra el token de autenticación al controlador de SQL Server.

La autenticación integrada solo está disponible para SQL Server. PostgreSQL y MySQL rechazan `--auth-mode integrated` con `DBP1005E`.

Los ejemplos anteriores presuponen que la entidad de seguridad de Windows ya existe como inicio de sesión de SQL Server. Los scripts de nivel de `sql/grants/` crean un inicio de sesión SQL con contraseña, que no es adecuado para este modo. Primero cree el inicio de sesión con `FROM WINDOWS` y después aplique sin cambios los permisos del nivel. Solo difiere el DDL del inicio de sesión. Consulte [Entidades de seguridad de Windows y de dominio para la autenticación integrada](../../sql/grants/DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication) para ver las instrucciones y los casos de grupos, cuentas de servicio administradas y cuentas de equipo.

Dos aspectos operativos son más importantes en este modo que con `sql-auth`. La cuenta con la que se ejecuta el proceso recopilador es la identidad que ve SQL Server. Si un administrador inicia el recopilador en un host donde `BUILTIN\Administrators` pertenece a `sysadmin`, la sesión es `sysadmin` y omite todas las reglas `DENY` del script de permisos aunque la captura se complete correctamente. `--expect-server-principal` hace que este caso falle con `DBP1606E` antes de cualquier lectura del catálogo. Además, una cuenta de servicio dedicada no hereda el acceso a archivos de quien la inició. Necesita permiso de lectura para su propio archivo de credenciales cuando se utilice uno, y permiso de escritura en las rutas de `--out` y `--audit-log`.

Cada conexión de SQL Server registra `ORIGINAL_LOGIN()`, `SUSER_SNAME()` y
`USER_NAME()` en la auditoría local. `--expect-server-principal` es opcional y
también funciona con autenticación SQL. SQL Server compara `ORIGINAL_LOGIN()`
con la entidad de seguridad esperada en la sesión establecida. Una discrepancia
o una identidad no disponible produce `DBP1606E` antes de cualquier captura del
catálogo. Las identidades exactas permanecen como evidencia de auditoría local
y no se incluyen en el Blueprint, la presentación ni los artefactos publicados.

## Autenticación de bases de datos administradas en la nube

Un punto de conexión administrado no cambia por sí solo los permisos de base de datos que requiere `dbwarp-blueprint`. Un nombre de usuario y una contraseña nativos usan `sql-auth` y no requieren un rol del plano de control de la nube una vez aprovisionados la red y la cuenta de base de datos.

`dbwarp-blueprint` no invoca CLI de la nube, servicios de metadatos, gestores de secretos ni API de renovación de tokens. Un wrapper debe generar o recuperar cada token de corta duración y suministrarlo mediante una sola fuente de secreto protegida.

### Tokens de nube para PostgreSQL y MySQL

Use `cloud-token` para un token directo de servicio administrado PostgreSQL o MySQL generado por AWS, Azure o Google Cloud. Proporcione exactamente una de las opciones `--password-file` o `--password-env`. El modo requiere `verify-full`; añada el paquete de CA del proveedor o de la instancia cuando no esté anclado en el conjunto de confianza compilado del binario.

Ejemplo de PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Ejemplo de MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Para MySQL, `cloud-token` habilita el intercambio `mysql_clear_password` únicamente dentro de esa conexión TLS verificada. Las conexiones `sql-auth` normales mantienen el complemento deshabilitado. PostgreSQL utiliza su protocolo de contraseña normal con el mismo requisito de TLS verificado.

### Permisos de ejecución en la nube

Estos permisos autorizan el inicio de sesión o un túnel de conexión; nunca sustituyen al principal ni a los permisos de la base de datos:

| Ruta administrada | Modo del binario | Permiso de ejecución fuera de la base de datos |
|---|---|---|
| Inicio de sesión IAM de RDS/Aurora PostgreSQL o MySQL | `cloud-token` | `rds-db:connect` sobre el ARN exacto del usuario de base de datos |
| Inicio de sesión Entra de Azure Database for PostgreSQL/MySQL | `cloud-token` | Ningún rol RBAC de recurso de Azure para acceder a los datos; la identidad debe estar asignada dentro de la base de datos |
| Inicio de sesión IAM directo de Cloud SQL PostgreSQL/MySQL | `cloud-token` | Permiso exacto `cloudsql.instances.login`; `roles/cloudsql.instanceUser` es la alternativa predefinida más amplia |
| Cloud SQL Auth Proxy o conector | Normalmente `sql-auth`; el proxy puede realizar autenticación IAM automática | La identidad del proxy necesita `roles/cloudsql.client`; la autenticación IAM automática también necesita permiso de inicio de sesión |
| Inicio de sesión Entra de Azure SQL Database o Managed Instance | `entra-token` | Ningún rol RBAC de recurso de Azure para acceder a los datos; use las opciones de token de SQL Server documentadas anteriormente |
| Cualquier base de datos administrada compatible con credenciales nativas | `sql-auth` | Ninguno |

La revisión de permisos del despliegue debe registrar los permisos de base de datos según la versión, las políticas exactas de la nube, las alternativas de roles integrados y sus salvedades de alcance. La configuración del proveedor, la creación de principales, el acceso a la red, la generación de tokens y la recuperación opcional de secretos son responsabilidades del aprovisionamiento o del wrapper; no son permisos que deban asociarse al recopilador solo porque el punto de conexión sea administrado.
