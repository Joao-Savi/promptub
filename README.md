# promptub

Player desktop de **YouTube / YouTube Music** para Windows — interface estilo terminal, leve e focada em música.

[![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6?logo=windows&logoColor=white)](https://github.com/Joao-Savi/promptub)
[![Tauri](https://img.shields.io/badge/Tauri-2-FFC131?logo=tauri&logoColor=black)](https://tauri.app)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## O que é

O **promptub** abre uma janela nativa pequena (~80–150 MB de RAM) para buscar, ouvir e montar filas — sem abrir o Chrome no dia a dia.

| Antes (v0.3.x) | Agora (v0.4.x) |
|----------------|----------------|
| Player **mpv** externo | **HTML5 `<audio>`** dentro do app |
| Tema verde terminal | **Noite:** preto + vermelho · **Dia:** branco + preto |
| Modo vídeo | Removido — só áudio |
| Feed monolítico | Feed **progressivo** por seção |

---

## Stack

| Camada | Tecnologia |
|--------|------------|
| Desktop | **Tauri 2** (Rust) |
| UI | **Vite 6** + HTML/CSS/JS vanilla |
| Stream / busca | **yt-dlp** (bundled no instalador) |
| Reprodução | `<audio>` WebView2 |
| Sessão Premium | **keyring** (Credential Manager) + cookies Edge |
| Letras | **LRCLIB** + legendas YouTube via yt-dlp |

---

## Funcionalidades

- Busca YouTube com `[ BUSCAR ]` ou Enter
- Feed inicial: Continuar ouvindo, Recomendados, Similares, Novos artistas, Feito pra você
- Histórico e cache de feed entre sessões (`%APPDATA%\promptub\`)
- Fila, playlist `[ REC.PL ]`, pré-aquecimento de stream
- Letras sincronizadas (LYRICS.SYS)
- Tema **noite** (vermelho) e **dia** (mono claro) — botão `dia`/`noite` na barra do player
- Login opcional `[ AUTH ]` para sessão YouTube Premium

---

## Instalação

### Usuário

1. Baixe `promptub_*_x64-setup.exe` em [Releases](https://github.com/Joao-Savi/promptub/releases) ou use:
   ```cmd
   INSTALAR.cmd
   ```
2. Instale — a reinstalação **remove legado** (mpv, dlls) e **limpa cache WebView2**
3. Atalho na área de trabalho: `scripts\install-shortcut.cmd` (ícone vermelho)

### Desenvolvedor

Requisitos: **Node.js 18+**, **Rust**, **MSVC Build Tools 2022**

```cmd
git clone https://github.com/Joao-Savi/promptub.git
cd promptub
npm install
scripts\build-tauri.cmd
```

Saída: `bin\promptub.exe` e `bin\promptub_*_setup.exe`

Ícone vermelho: `scripts\generate-icon.ps1`

---

## Estrutura

```
promptub/
├── src/                    # Frontend (HTML, CSS, JS)
├── src-tauri/src/          # Backend Rust
│   ├── feed_sections.rs    # Feed progressivo
│   ├── music_recommend.rs  # Recomendações
│   ├── lyrics.rs           # Letras LRCLIB/YouTube
│   ├── history.rs          # Histórico persistente
│   └── stream.rs           # Cache de URLs
├── scripts/                # Build, instalador, ícone
├── scripts/legacy/         # Testes antigos mpv (dev)
├── bin/                    # Build local (exe + setup)
├── CHANGELOG.md            # Histórico detalhado
├── VERSION.md              # Regras SemVer
└── INSTALAR.cmd
```

---

## Comandos úteis

| Comando | Ação |
|---------|------|
| `INSTALAR.cmd` | Abre o instalador mais recente |
| `scripts\build-tauri.cmd` | Compila exe + setup |
| `dev.cmd` | Dev com hot reload |
| `scripts\install-shortcut.cmd` | Atalho desktop com ícone |
| `scripts\pos-instalacao.cmd` | Desbloqueia + limpa cache |

---

## Versão

Versão atual: **0.4.2** — ver [CHANGELOG.md](CHANGELOG.md) e [VERSION.md](VERSION.md).

---

## Licença

MIT — uso pessoal. YouTube é serviço de terceiros; respeite os Termos de Uso do Google.

Desenvolvido por [Joao-Savi](https://github.com/Joao-Savi)
