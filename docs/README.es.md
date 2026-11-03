<p align="center">
  <img src="https://cdn.jsdelivr.net/gh/jub0t/Concat@main/assets/concat-mark-dark.png" alt="Concat" width="100" />
</p>

<h1 align="center">Concat</h1>
<h3 align="center">El reemplazo de CapCut gratuito, de código abierto y multiplataforma.</h3>
<h4 align="center">✨ Versión Mejorada con Asistente Google Gemini AI, Zoom Inteligente con Rostros y CI/CD Multiplataforma ✨</h4>

> [!NOTE]
> **Versión Mejorada**: Este repositorio es una bifurcación (*fork*) mejorada basada en el software original [Concat creado por Jareer (jub0t)](https://github.com/jub0t/Concat). Incorpora un **Asistente Inteligente Google Gemini** interactivo (con OAuth 2.0 y API Key propia), **Efecto Ken Burns con Detección de Rostros por IA**, **Expansión de Escena Vertical a Horizontal (Outpainting)** y compilación automatizada para Windows y Android — manteniendo de forma estricta la misma licencia libre **GNU AGPLv3**.

<div align="center">

<img src="https://cdn.jsdelivr.net/gh/jub0t/Concat@main/assets/editor.png" alt="Concat editor" width="100%" />

<p align="center">
  <a href="https://github.com/fernandoariasprogramador-code/ConCat/actions/workflows/build-windows-and-android.yml"><img src="https://github.com/fernandoariasprogramador-code/ConCat/actions/workflows/build-windows-and-android.yml/badge.svg" alt="Estado de Compilación" /></a>
  <a href="https://github.com/fernandoariasprogramador-code/ConCat/actions"><img src="https://img.shields.io/badge/Compilaciones-Windows%20%26%20Android-success?style=flat&logo=android&logoColor=F8F8F8&labelColor=212123" alt="Compilaciones Automáticas" /></a>
  <a href="../LICENSE"><img src="https://img.shields.io/badge/Licencia-AGPL%20v3-0568FD?style=flat&logo=gnu&logoColor=F8F8F8&labelColor=212123" alt="Licencia: AGPL-3.0-or-later" /></a>
</p>

<p align="center">
  <a href="https://github.com/fernandoariasprogramador-code/ConCat/actions/runs/41655722087"><img src="https://img.shields.io/badge/Descargar%20APK-Android%20(arm64)-3DDC84?style=for-the-badge&logo=android&logoColor=white" alt="Descargar Android APK" /></a>
  &nbsp;&nbsp;
  <a href="https://github.com/fernandoariasprogramador-code/ConCat/actions/runs/41655722087"><img src="https://img.shields.io/badge/Descargar%20Instalador-Windows%20(x64)-0078D6?style=for-the-badge&logo=windows&logoColor=white" alt="Descargar Windows x64" /></a>
</p>

[🇬🇧 English](../README.md) · [🇩🇪 Deutsch](README.de.md) · **🇪🇸 Español** · [🇫🇷 Français](README.fr.md) · [🇮🇹 Italiano](README.it.md) · [🇳🇱 Nederlands](README.nl.md) · [🇵🇱 Polski](README.pl.md) · [🇧🇷 Português (Brasil)](README.pt-BR.md) · [🇷🇺 Русский](README.ru.md) · [🇺🇦 Українська](README.uk.md) · [🇹🇷 Türkçe](README.tr.md) · [🇭🇷 Hrvatski](README.hr.md) · [🇮🇩 Bahasa Indonesia](README.id.md) · [🇻🇳 Tiếng Việt](README.vi.md) · [🇹🇭 ไทย](README.th.md) · [🇯🇵 日本語](README.ja.md) · [🇰🇷 한국어](README.ko.md) · [🇨🇳 简体中文](README.zh-Hans.md) · [🇹🇼 繁體中文](README.zh-TW.md) · [🇸🇦 العربية](README.ar.md) · [🇮🇱 עברית](README.he.md) · [🇮🇷 فارسی](README.fa.md) · [🇮🇳 हिन्दी](README.hi.md) · [🇵🇰 اردو](README.ur.md) · [🇧🇩 বাংলা](README.bn.md)

</div>


## Acerca de

Concat es un editor de vídeo gratuito y de código abierto, una alternativa a CapCut para macOS, Windows, Linux y Android. Cubre aquello para lo que la gente realmente abre CapCut: subtítulos automáticos, texto a voz, eliminación de fondo, animación por fotogramas clave, efectos y títulos, corte multipista, exportación 4K. Sin trampas: sin marca de agua, sin cuenta, sin suscripción, sin subir nada.

Todo corre en local sobre un motor nativo en Rust con un compositor por GPU. Instálalo, suelta el material, corta. Los modelos de IA para subtítulos, voces y recorte se descargan una vez desde Ajustes y después funcionan sin conexión. Tu material nunca sale de tu disco.

**Bueno para:** TikTok, Reels y Shorts, vídeos de YouTube, tutoriales y grabaciones de pantalla, clips de pódcast, memes.

**También para máquinas:** una API JSON-RPC, para que los scripts y los agentes de IA también puedan cortar vídeo con él.

## Lo más destacado

- 🚫 **Sin marcas de agua. Sin cuenta. Sin muro de pago.** Nunca.
- 🔒 **100 % local.** Nada se sube. Funciona sin conexión.
- 💬 **Subtítulos automáticos.** Whisper en local. Elige un tamaño de modelo y obtén subtítulos con estilo en la línea de tiempo.
- 🗣️ **Texto a voz + clonación de voz.** Voces locales gratuitas, o cualquier voz a partir de unos segundos de grabación.
- 🧍 **Eliminación de fondo.** Personas, objetos, o pinta la máscara tú mismo.
- 🎞️ **Fotogramas clave.** Posición, escala, rotación, opacidad, volumen, parámetros de efectos. Editor de curvas integrado.
- ✨ **100 efectos, filtros y transiciones.** Renderizados por GPU, en vivo en la vista previa.
- ✂️ **Corta rápido.** Dividir, recortar, ripple, unir, congelar fotograma, velocidad. Línea de tiempo magnética si la quieres.
- 🎚️ **Multipista, multilínea de tiempo.** Varios montajes en un proyecto. Modos de fusión, recorte, volteos.
- 📝 **Títulos.** Fuentes, trazo, sombra, placa de fondo. Preajustes para empezar.
- 🎙️ **Limpieza de voz con un interruptor.** Reducir ruido, realzar la voz, nivelar el volumen. Más ardilla, robot, teléfono y compañía.
- 📤 **Exportación.** H.264, HEVC, AV1. Hasta 4K 60, color de 10 bits.
- 🦀 **Motor nativo en Rust.** Compositor por GPU, proxies, decodificación por hardware. El 4K se desplaza con fluidez.
- 🤖 **Programable.** Una API JSON-RPC con la que los scripts y los agentes de IA pueden cortar vídeo. Compilando desde el código fuente se añaden gRPC y una herramienta de línea de comandos.
- 🖥️ **macOS, Windows, Linux, Android.** 25 idiomas. La misma app, los mismos archivos de proyecto.

## 🚀 Novedades en esta versión mejorada

Esta versión incorpora herramientas de inteligencia artificial generativa, visión por computadora y flujos de compilación automatizados en la nube:

### 🤖 1. Asistente de Vídeo con Google Gemini AI
- **Control del editor por lenguaje natural**: Pídele al asistente que busque silencios, corte tomas, aplique transiciones, reordene fragmentos o ajuste niveles de audio mediante llamadas a funciones (*Function Calling*).
- **Compatibilidad con modelos avanzados**: Soporta `gemini-2.5-flash`, `gemini-2.5-pro` y modelos *preview*.
- **Usa tu propia cuenta/clave (BYOK) y Privacidad**: Cada usuario puede iniciar sesión mediante **Google OAuth 2.0 PKCE** o ingresar su propia clave de Google AI Studio (`/key AIza...`). Cada persona consume únicamente su propia cuota gratuita o suscripción sin compartir tokens.
- **Interfaz interactiva**: Incluye una burbuja flotante arrastrable y un panel de asistente acoplable con *Modo Copiloto* (pide confirmación antes de editar) y *Modo Autónomo*.

### 👤 2. Efecto Ken Burns con Detección de Rostros por IA
- **Seguimiento facial neuronal local**: Modelo de red neuronal ONNX ultra-rápido (UltraFace) integrado que detecta rostros humanos en CPU/GPU sin enviar imágenes a internet.
- **Encuadre cinemático inteligente**: Centra la toma de forma suave en los sujetos creando un efecto de paneo y zoom progresivo sobre el rostro.
- **Personalizable**: Ajusta el nivel de zoom, la curva de aceleración y la duración.

### 🖼️ 3. Expansión de Escena con IA (Outpainting)
- **Adaptación de aspecto vertical a horizontal**: Convierte vídeos y fotos verticales (9:16) en formato panorámico (16:9).
- **Múltiples modos de relleno**: Fondo generado por IA, reflejo simétrico o desenfoque bokeh suave.

### 📦 4. Compilaciones Automáticas Multiplataforma (Windows y Android APK)
- **Flujo CI/CD automatizado**: Mediante GitHub Actions (`build-windows-and-android.yml`).
- **Descarga directa de artefactos**: Genera automáticamente el instalador de Windows (Setup `.exe` y ZIP portable) y el archivo APK para Android ARM64 en la pestaña de [Actions](https://github.com/fernandoariasprogramador-code/ConCat/actions).

## 📥 Descargar Compilaciones

Puedes descargar los binarios precompilados directamente desde el flujo de compilación automatizado de este repositorio:

| Plataforma | Formato | Descarga Directa / Artefactos |
| :--- | :--- | :--- |
| 📱 **Android** (arm64) | Instalador APK (`.apk`) | **[Descargar APK de Android (ARM64)](https://github.com/fernandoariasprogramador-code/ConCat/actions/runs/41655722087)** *(Sección: Artifacts)* |
| 💻 **Windows** (x86_64) | Setup (`.exe`) y Portable (`.zip`) | **[Descargar Instalador de Windows x64](https://github.com/fernandoariasprogramador-code/ConCat/actions/runs/41655722087)** *(Sección: Artifacts)* |

> [!TIP]
> Todas las compilaciones nuevas y futuras se generan de forma continua y están disponibles en la **[pestaña Actions](https://github.com/fernandoariasprogramador-code/ConCat/actions)**.

## Cómo contribuir

> [!IMPORTANT]
> La mejor forma de contribuir es descargar una compilación de la página de [Releases](https://github.com/jub0t/Concat/releases) y usarla: encuentra dónde falla y di dónde podría ser mejor.
>
> ¿Listo para escribir código? [CONTRIBUTING.md](../CONTRIBUTING.md) cubre la configuración, la disposición del árbol, las comprobaciones que ejecutar y cómo se licencian las contribuciones. ¿Controlas Concat desde un script, un servicio o un agente? [La documentación para desarrolladores](https://concatenate.pages.dev/docs) cubre la API de Concat y sus transportes: JSON-RPC y gRPC. [Esta Discussion](https://github.com/jub0t/Concat/discussions/3) es donde se anunció el proyecto.
> 
> Los colaboradores pueden reclamar un rol `@Contributor` en el servidor de Discord: solo hay que pedirlo.

## 📄 Licencia y Reconocimiento

Este proyecto es software libre publicado bajo la **GNU Affero General Public License Versión 3 (AGPL-3.0-or-later)** — exactamente la misma licencia libre y copyleft del proyecto original. Eres libre de usar, estudiar, modificar y redistribuir este programa bajo los términos de la licencia AGPLv3.

- **Proyecto y autor original:** [Concat por Jareer (jub0t)](https://github.com/jub0t/Concat).
- **Modificaciones y versión mejorada:** Mantenido por [Fernando Arias](https://github.com/fernandoariasprogramador-code/ConCat).
- **Texto completo de la licencia:** Consulta el archivo [LICENSE](../LICENSE).

## DMCA

Si crees que algo de este repositorio, de sus versiones publicadas o de un paquete o modelo que distribuye infringe tus derechos de autor, escribe a [jub0trd@gmail.com](mailto:jub0trd@gmail.com) con una descripción de la obra que te pertenece, un enlace al lugar donde aparece aquí el material infractor, tus datos de contacto, una declaración de que crees de buena fe que el uso no está autorizado por ti, por tu representante ni por la ley, y una declaración, bajo pena de perjurio, de que la notificación es exacta y de que eres el titular o estás autorizado para actuar en su nombre, firmada con tu firma física o electrónica. Las notificaciones válidas se atienden con prontitud, y cualquier persona cuyo material sea retirado puede enviar una contranotificación a la misma dirección.

