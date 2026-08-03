# Changelog — promptub

Histórico de versões com **como era antes** e **o que mudou**.

---

## [0.5.0] — 2026-08-03

### Algoritmo da fila e gosto
- **Antes:** fila repetia mesma faixa/artista; podia misturar generos; sem preferencias explicitas
- **Agora:** like/dislike (♥/✕) na barra do player; fila usa genero, artistas parecidos e historico; bloqueia nao-musica e generos fora do contexto; deduplica por titulo normalizado

---

## [0.4.3] — 2026-08-03

### Atalho desktop
- **Antes:** atalho usava icone embutido verde do exe antigo (`IconLocation` vazio)
- **Agora:** `promptub.ico` vermelho instalado junto; instalador e `install-shortcut.cmd` forcam icone vermelho no atalho

---

## [0.4.2] — 2026-08-03

### UI / tema
- **Antes:** botão `[ DIA ]` grande na sidebar; tema dia vermelho/rosa
- **Agora:** botão compacto `dia`/`noite` ao lado dos atalhos de teclado na barra do player
- **Noite:** preto + vermelho (igual v0.4.1)
- **Dia:** branco/cinza + preto (sem vermelho — confortável para os olhos)
- Ícone do app e atalho desktop **vermelho** (`scripts/generate-icon.ps1`)

---

## [0.4.1] — 2026-08-03

### Tema e instalador
- **Antes:** tema verde terminal; reinstalar deixava mpv/dlls legados; WebView2 cacheava CSS antigo
- **Agora:** tema vermelho (noite); instalador mata processo, apaga legado, recria `tools/`, limpa `%LOCALAPPDATA%\com.promptub`
- CSS inline no HTML garante cor correta
- Segurança: validação URLs stream, CSP, XSS na busca, proteção `cookies.txt`

---

## [0.4.0] — 2026-08-03 — MINOR

### Performance e UX
- **Antes:** feed carregava tudo de uma vez com overlay; buscas sequenciais; player mpv
- **Agora:**
  - Feed **progressivo** (`home_feed_local` + `home_feed_section`)
  - Buscas yt-dlp **paralelas**
  - Cache de stream em **disco**
  - Player **HTML5** (sem mpv)
  - Logo promptub → refresh essencial (só Recomendados)
  - Prewarm das primeiras faixas; letras lazy (2s)
  - Histórico + cache feed (~36h) entre sessões

### Removido
- `ipc.rs`, `video_embed.rs`, `video_discover.rs`, `video_recommend.rs`
- Comandos stubs mpv: `warmup`, `stop`, `get_volume`, `set_volume`, `boot_mode`
- `home_recommendations` monolítico (substituído por feed por seção)

---

## [0.3.16] — PATCH

- **Antes:** letras falhavam (ex.: Jorge & Mateus — Paredes)
- **Agora:** LRCLIB primeiro; flags yt-dlp corrigidas; parse artista/título

---

## [0.3.12] — PATCH

- **Antes:** app **travava** ao clicar ⏭ (deadlock `queue.lock` + `track_play`)
- **Agora:** lock da fila solto antes de `track_play()`

---

## [0.3.13] — MINOR

- **Antes:** histórico só na sessão; feed sempre refetch
- **Agora:** `history.json` + `feed_cache.json` persistentes

---

## [0.3.8] — MINOR

- Feed por seções estilo YouTube Music
- Diversidade de artistas no refill da fila
- Cold start global para novos usuários

---

## [0.3.0 – 0.3.7] — era mpv

Como era o app nesta fase:

| Aspecto | Comportamento |
|---------|----------------|
| Player | **mpv.exe** embutido (áudio e vídeo) |
| UI | Tema **verde** terminal (`#33ff66`) |
| Modos | `audio.sys` e `video.sys` |
| Instalador | yt-dlp + mpv + DLLs |
| RAM | ~80–150 MB + processo mpv |

Transição para v0.4.x removeu mpv, modo vídeo e tema verde.

---

## Formato de versão

Ver [VERSION.md](VERSION.md) — SemVer `MAJOR.MINOR.PATCH`.
