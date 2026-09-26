# Decisiones de arquitectura (ADR)

Cada archivo registra una decisión importante: el contexto, lo que se decidió, las alternativas y lo que implica. Sirve para que nadie la deshaga sin saber por qué se tomó.

Se escribe un ADR solo si la decisión cumple las tres condiciones:

1. **Es difícil de revertir** (cambiarla después cuesta).
2. **Sorprendería sin contexto** (alguien preguntaría "¿por qué así?").
3. **Hubo alternativas reales** y se eligió una por motivos concretos.

Los ADR no se editan después de aceptados. Si la decisión cambia, se escribe uno nuevo que diga "Reemplaza a 000X" y el viejo se marca como reemplazado.

| # | Decisión | Estado |
|---|---|---|
| [0001](0001-interfaz-web-webview2.md) | La interfaz es una página web en WebView2 | Aceptada |
| [0002](0002-secuencias-verificadas.md) | Solo secuencias de Synapse/OpenRazer, con candado y verificación por lectura | Aceptada |
| [0003](0003-thx-por-sus-ajustes.md) | THX se controla escribiendo sus ajustes, sin redistribuir su driver | Aceptada (el camino lo elige 0004) |
| [0004](0004-thx-por-com.md) | Los ajustes de THX se cambian por la interfaz COM de su servicio | Aceptada (ampliada por 0005) |
| [0005](0005-thx-por-zeromq.md) | Lo que COM no puede cambiar en THX va por ZeroMQ, con un cliente propio | Aceptada |
| [0006](0006-eq-como-synapse.md) | El preset del headset elige también el preset y la curva de THX | Aceptada |
| [0007](0007-microfono-en-el-perfil.md) | Las mejoras del micrófono se guardan en el perfil y rzr se las vuelve a mandar a THX | Aceptada |

## Plantilla

```markdown
# 000X. Título en forma de decisión

- Estado: Propuesta | Aceptada | Reemplazada por 000Y
- Fecha: AAAA-MM-DD

## Contexto
Qué problema había y qué restricciones.

## Decisión
Qué se decidió, en una o dos frases claras.

## Alternativas
Qué más se consideró y por qué no.

## Consecuencias
Qué se gana, qué se pierde y qué hay que cuidar desde ahora.
```
