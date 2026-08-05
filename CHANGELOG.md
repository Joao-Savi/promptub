# Changelog — promptub

Histórico de versões com **como era antes** e **o que mudou**.

---

## [0.5.12] — 2026-08-05

### Karaoke: precisao e velocidade
- **Antes:** match LRCLIB fraco podia vencer; proxima faixa sem letra pronta; sync fixo
- **Agora:** prefere legendas do video se match < 75%; prefetch na fila; slider sync ±0,5 s; timeout LRCLIB 3 s

---

## [0.5.11] — 2026-08-05

### Letras: validacao e karaoke confiavel
- **Antes:** LRCLIB devolvia a 1ª letra da busca (musica errada); letra plain estimada
- **Agora:** confere artista+faixa (score ≥ 0,65); so letra sincronizada real; LRCLIB e YouTube em paralelo; rejeita legendas vazias/lixo

---

## [0.5.10] — 2026-08-05

### Letras: velocidade e sync
- **Antes:** yt-dlp baixava legendas YouTube primeiro (15–30 s); letra adiantada vs audio
- **Agora:** LRCLIB primeiro (~1 s); cache local; busca ao dar play; lag fino por fonte (YouTube −0,55 s)

---

## [0.5.9] — 2026-08-05

### Stream e sync de letras
- **Antes:** reconexao em loop no buffer; letra adiantada 0,5 s
- **Agora:** reconecta so se travar 20+ s (nao ao pausar); legendas YouTube primeiro; timestamps exatos

---

## [0.5.8] — 2026-08-05

### Limpeza automatica de cache
- **Antes:** stream_cache acumulava dezenas de URLs; temp de letras ficava no disco
- **Agora:** max 16 URLs (20 min); limpeza ao abrir e a cada 30 min; temp promptub-* apos 1 h; feed 24 h

---

## [0.5.7] — 2026-08-05

### Stream, letras e sync
- **Antes:** musica travava sem recuperar; letras com lixo de legenda auto; sync ~0,5 s atrasada
- **Agora:** reconexao automatica do stream; LRCLIB primeiro + filtro de junk; antecipacao de 0,5 s na letra sincronizada

---

## [0.5.6] — 2026-08-04

### Sync karaoke das letras
- **Antes:** letra adiantava antes da voz (lead artificial + plain desde 0 s)
- **Agora:** legendas do YouTube primeiro; timestamps exatos; intro sem destaque; letras plain com estimativa de entrada vocal

---

## [0.5.5] — 2026-08-04

### Letras e fila automatica
- **Antes:** letra ia atrasando ao longo da musica; tocar uma faixa sozinha nao gerava fila
- **Agora:** letras plain redistribuidas pela duracao + compensacao de drift; fila preenche com mix do YouTube na primeira musica

---

## [0.5.4] — 2026-08-04

### Sincronia das letras e importar playlist
- **Antes:** destaque da letra atrasava a voz; importar mix/radio (`RD...`) falhava no yt-dlp
- **Agora:** antecipacao de 0,45 s + sync a 60 fps; URLs de radio/mix e playlists PL corrigidas; `--ignore-errors` na importacao

---

## [0.5.3] — 2026-08-04

### Letras, karaoke e importar playlist
- **Antes:** letras nunca apareciam (comando Tauri errado); sem modo karaoke; sem importar playlist do YouTube na fila
- **Agora:** letras via LRCLIB (HTTP nativo) + fallback legendas YouTube; botão `[ Karaoke ]` amplia a letra na tela; importar playlist YouTube acima da fila (até 100 faixas)

---

## [0.5.2] — 2026-08-03

### Recomendados e nomes simples
- **Antes:** recomendados seguiam busca solta (ex. "panda"); botoes REC.PL, ART.PL, DESC.PL
- **Agora:** recomendados pelo genero do historico; botoes `[ Pra você ]`, `[ Do artista ]`, `[ Misturado ]`, `[ Novidades ]`; secao Descobertas com lancamentos do genero

---

## [0.5.1] — 2026-08-03

### Fila, playlists e generos
- **Antes:** artista repetia no maximo 1x; sem playlist por artista; generos misturados
- **Agora:** ate 2 faixas diferentes por artista na fila; `[ ART.PL ]` (playlist do artista); `[ MIX ]` (misturadao); secoes separadas por genero no feed

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
