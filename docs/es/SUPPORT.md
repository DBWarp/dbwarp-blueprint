# Soporte e informes de problemas

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa y puede contener errores. No debe considerarse texto apto para uso contractual. Consulte el [documento canónico en inglés](../../SUPPORT.md).

**Idiomas:** [English](../../SUPPORT.md) | [Deutsch](../de/SUPPORT.md) | [Français](../fr/SUPPORT.md) | **Español** | [Polski](../pl/SUPPORT.md) | [日本語](../ja/SUPPORT.md) | [简体中文](../zh/SUPPORT.md)

Utilice el [gestor de incidencias](https://github.com/DBWarp/dbwarp-blueprint/issues)
para preguntas de instalación sin información sensible, defectos reproducibles y
solicitudes de funciones. Este canal no promete un tiempo de respuesta ni un
nivel de servicio de soporte.

Notifique las posibles vulnerabilidades en privado mediante la vía indicada en
[SECURITY.md](SECURITY.md), no mediante una incidencia pública.

## Información útil

- Etiqueta exacta de la versión, suma de comprobación del binario, sistema
  operativo y arquitectura.
- Motor y versión de la base de datos, o formato del archivo estructurado, y si
  el origen es autogestionado o administrado.
- Opciones del comando, eliminando las credenciales, los puntos de conexión, las
  rutas y los selectores identificativos, o sustituyéndolos por ejemplos
  claramente señalados.
- Código de diagnóstico `DBP`, comportamiento esperado y comportamiento real.
- Una reproducción sintética pequeña, cuando sea posible.

No cargue datos de producción, credenciales ni auditorías, paquetes, Blueprints o
presentaciones sin revisar. Las auditorías pueden contener identidades y puntos
de conexión; incluso los Blueprints anónimos pueden revelar una estructura de
carga de trabajo distintiva. Empiece con la descripción segura mínima y revise
cada archivo adjunto antes de compartirlo.

## Configuraciones admitidas

[STATUS.md](../../STATUS.md) describe las capacidades y la matriz de motores
validados. [BUILD.md](BUILD.md) describe los requisitos de compilación específicos
de plataforma y autenticación. Rust está fijado a la versión exacta de
`rust-toolchain.toml`; el `rust-version` del paquete no promete que todas las
cadenas de herramientas posteriores estén validadas.

Las instrucciones de permisos para servicios administrados no afirman que se
hayan probado todos los servicios o configuraciones. Utilice los
[requisitos de permisos](../../sql/grants/DATABASE_PERMISSIONS.md) correspondientes
y valide la configuración exacta antes de utilizarla en producción.

Para los cambios entre versiones del recopilador, consulte [CHANGELOG.md](CHANGELOG.md).
