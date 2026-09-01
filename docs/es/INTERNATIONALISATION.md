# Internacionalización

> **Aviso de traducción:** Esta es una traducción asistida por máquina pendiente de revisión técnica por una persona nativa. No debe considerarse redacción apta para uso contractual. Consulte el [documento canónico en inglés](../INTERNATIONALISATION.md).

**Idiomas:** [English](../INTERNATIONALISATION.md) | [Deutsch](../de/INTERNATIONALISATION.md) | [Français](../fr/INTERNATIONALISATION.md) | **Español** | [Polski](../pl/INTERNATIONALISATION.md) | [日本語](../ja/INTERNATIONALISATION.md) | [中文](../zh/INTERNATIONALISATION.md)

`dbwarp-blueprint` separa la presentación dirigida a personas de la sintaxis operativa. Se trata de un
límite de seguridad y automatización, no solo de una preferencia de visualización.

## Idiomas admitidos

El texto fuente en inglés es la referencia autoritativa. Los catálogos de presentación en otros idiomas están asistidos por máquina y pueden contener errores aunque se valide su cobertura de claves y tokens. Compare con el texto en inglés las decisiones de seguridad, contractuales, normativas y de mínimo privilegio. Consulte [`TRANSLATIONS.md`](../TRANSLATIONS.md) para conocer el proceso independiente de publicación de documentos traducidos.

| Valor | Idioma | Etiqueta de configuración regional utilizada en las presentaciones generadas |
|---|---|---|
| `en` | Inglés | `en-US` |
| `de` | Alemán | `de-DE` |
| `fr` | Francés | `fr-FR` |
| `es` | Español | `es-ES` |
| `pl` | Polaco | `pl-PL` |
| `ja` | Japonés | `ja-JP` |
| `zh` | Chino simplificado | `zh-CN` |

Seleccione un idioma explícitamente:

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

Cuando no se especifica `--lang`, el orden de resolución es:

1. `DBWARP_BLUEPRINT_LANG`;
2. `LC_ALL`;
3. `LC_MESSAGES`;
4. `LANG`;
5. inglés.

Se aceptan sufijos de región y codificación en las etiquetas de configuración regional del entorno, por lo que
`de_CH.UTF-8`, `pl_PL.UTF-8` y `ja-JP` se resuelven a su idioma base.
Los valores explícitos de `--lang` se limitan deliberadamente a los siete tokens
canónicos de la tabla.

En Windows, `LC_ALL`, `LC_MESSAGES` y `LANG` no suelen estar definidos, por lo que la herramienta usa inglés salvo que se indique `--lang` o `DBWARP_BLUEPRINT_LANG`, por ejemplo `$env:DBWARP_BLUEPRINT_LANG = "de"` en PowerShell o `set DBWARP_BLUEPRINT_LANG=de` en cmd. Los nombres de variables no distinguen mayúsculas en Windows, pero sí en Linux y macOS; utilice siempre los nombres canónicos en mayúsculas.

## Qué se traduce

- descripciones de ayuda del nivel superior y de las opciones;
- estructura de la ayuda, como las etiquetas de uso y de valores posibles;
- planes de comprobación previa y solicitudes de consentimiento;
- resumen, causa y acción correctiva de los mensajes DBP;
- texto de progreso y advertencias;
- encabezados, etiquetas, explicaciones y metadatos de configuración regional de las presentaciones de PowerPoint.

Los detalles técnicos fatales pueden permanecer literales bajo el mensaje DBP localizado cuando sean necesarios para el diagnóstico. Las advertencias no fatales ocultan los detalles brutos del controlador cuando podrían contener identificadores de origen; el código DBP estable y el destino Blueprint anónimo permanecen disponibles.

## Qué nunca cambia

Los siguientes elementos se mantienen como tokens canónicos en inglés en todos los idiomas de presentación:

- el comando `dbwarp-blueprint` y nombres de opciones como `--measure-compression`;
- valores aceptados como `verify-full`, `balanced` y `exact`;
- esquemas de URI como `postgresql://`, `mysql://` y `sqlserver://`;
- nombres de variables de entorno y rutas de archivos;
- selectores como `source=ID` y `table=ID`;
- identificadores DBP como `DBP1001E`;
- identificadores anonimizados como `table-001`, `col-1` y `schema-A`;
- claves de auditoría, claves TOML, claves de paquetes, nombres de tipos de bases de datos y métodos de índices.

Por lo tanto, los scripts no necesitan un tratamiento específico por idioma de las opciones o los valores,
y un Blueprint generado con `--lang ja` es idéntico byte a byte a otro generado
con `--lang en` cuando todas las demás entradas deterministas son iguales.

## Comportamiento estricto de los catálogos

Todos los catálogos se compilan dentro del binario. Al iniciar, el programa verifica
que cada configuración regional no inglesa anunciada cubra exactamente:

- el árbol de ayuda Clap activo en ese momento;
- todos los códigos DBP estables y los tres campos de diagnóstico;
- todas las claves estables de solicitudes, progreso, advertencias y presentaciones;
- todos los marcadores de posición y tokens operativos protegidos necesarios.

Las entradas ausentes o adicionales, los cambios de marcadores de posición, los tokens operativos alterados,
el JSON no válido o los controles de formato invisibles o bidireccionales hacen que el programa se detenga y
rechace el catálogo con `DBP1010E`. El programa no sustituye silenciosamente por inglés una traducción ausente.

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
