# Versionamento do promptub

SemVer: `MAJOR.MINOR.PATCH`

| Parte | Quando subir |
|-------|----------------|
| **MAJOR** | Mudança grande, quebra compatibilidade (ex.: 0.x → 1.0) |
| **MINOR** | Funcionalidade nova (feed progressivo, letras, temas) |
| **PATCH** | Correção, performance, UI, segurança |

## Onde atualizar ao lançar

- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `package.json`
- `INSTALAR.cmd` → `WANT`
- `CHANGELOG.md`

## Versão atual: **0.5.1**

| Versão | Tipo | Resumo |
|--------|------|--------|
| **0.5.1** | PATCH | ART.PL, MIX, generos separados no feed; fila permite 2 faixas/artista |
| **0.5.0** | MINOR | Like/dislike; algoritmo de fila personalizado por genero e gosto |
| **0.4.3** | PATCH | Atalho desktop com icone vermelho explicito |
| **0.4.1** | PATCH | Tema vermelho; instalador limpo; segurança |
| **0.4.0** | MINOR | HTML5, feed progressivo, sem mpv |
| **0.3.x** | — | Era mpv + verde + vídeo (ver CHANGELOG) |

Detalhes completos: [CHANGELOG.md](CHANGELOG.md)
