# Contribuir

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa y puede contener errores. No debe considerarse texto apto para uso contractual. Consulte el [documento canónico en inglés](../../CONTRIBUTING.md).

**Idiomas:** [English](../../CONTRIBUTING.md) | [Deutsch](../de/CONTRIBUTING.md) | [Français](../fr/CONTRIBUTING.md) | **Español** | [Polski](../pl/CONTRIBUTING.md) | [日本語](../ja/CONTRIBUTING.md) | [简体中文](../zh/CONTRIBUTING.md)

Empiece con una incidencia sin información sensible que describa el problema y
una reproducción sintética pequeña. Para cambios importantes, comente el enfoque
antes de preparar un parche. Los responsables del mantenimiento deciden si una
propuesta encaja con el producto y sus límites de seguridad; abrir una incidencia
o una solicitud de incorporación de cambios no implica su aceptación.

Siga [SUPPORT.md](SUPPORT.md) para informar de problemas de forma segura y
[SECURITY.md](SECURITY.md) para notificar vulnerabilidades en privado. Mantenga
un trato respetuoso y centre las conversaciones en comportamientos reproducibles.

## Preparar un cambio

- Utilice la cadena de herramientas fijada y las dependencias bloqueadas que se
  describen en [BUILD.md](BUILD.md).
- Limite el alcance de los cambios y añada cobertura de regresión para los
  comportamientos modificados.
- Nunca incluya datos de clientes, credenciales, detalles de infraestructura
  privada ni evidencia de auditoría identificativa en un parche o conjunto de
  datos de prueba.
- Preserve el consentimiento explícito, el impacto acotado sobre el origen, el
  acceso con privilegios mínimos y la información veraz sobre observaciones
  ausentes o degradadas.
- Documente cualquier cambio del contrato Blueprint o de la codificación de
  compresión. Las reglas existentes sobre campos opcionales y compatibilidad
  forman parte de la interfaz.
- Mantenga las opciones CLI, los códigos de diagnóstico y los campos serializados
  en inglés canónico. Los cambios de ejecución visibles para personas deben
  actualizar todos los catálogos de ejecución distribuidos. El Markdown en inglés
  es la referencia autoritativa; el Markdown traducido es complementario.

## Comprobaciones locales

Desde el repositorio de código fuente, con la cadena de herramientas fijada
instalada:

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

El núcleo compartido tiene su propia suite de pruebas unitarias:

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

El éxito de las pruebas locales no valida versiones de bases de datos ni
plataformas. Describa lo que se probó realmente y marque explícitamente las
demás configuraciones como no probadas. No publique artefactos, actualice
etiquetas de versión ni cambie los ajustes de seguridad del repositorio como
parte de un parche sin la aprobación de los responsables del mantenimiento.

## Flujo de trabajo de mantenimiento

La fuente canónica es la ayuda en inglés de Rust y las definiciones de mensajes e interfaz de usuario
en `src/i18n.rs`. Cuando cambia cualquier frase visible para el cliente:

1. actualice cada catálogo de configuración regional bajo `locales/` en el mismo commit;
2. conserve exactamente todos los marcadores de posición y tokens operativos canónicos;
3. ejecute la prueba específica de cobertura exacta;
4. añada o actualice el caso pertinente del límite del operador en
   `tests/cli_errors.rs` cuando cambie un error o una advertencia;
5. ejecute todo el conjunto de pruebas e inspeccione resultados representativos de ayuda y presentaciones;
6. obtenga una revisión técnica por una persona nativa antes de considerar definitivo el nuevo texto para un
   contrato de cliente, una presentación reglamentaria o material público de marketing.

Este proceso de cobertura exacta se aplica a los catálogos de ejecución
integrados en el binario. El Markdown traducido es complementario; consulte
[`docs/TRANSLATIONS.md`](../TRANSLATIONS.md).

Validación específica:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

Las pruebas de integración también demuestran que los tokens de opciones son idénticos en todos los
idiomas, que los códigos DBP localizados se mantienen estables, que el TOML emitido no varía según el idioma
y que el contenido generado de la presentación lleva la configuración regional seleccionada.
