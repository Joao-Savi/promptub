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
- `README.md`
- `index.html` (banner welcome)

## Versão atual: **0.5.8**

| Versão | Tipo | Resumo |
|--------|------|--------|
| **0.5.8** | PATCH | Limpeza automatica de cache (disco e temp) |
| **0.5.7** | PATCH | Reconexao de stream; filtro de letras junk; lead 0,5 s |
| **0.5.6** | PATCH | Sync karaoke: legendas YouTube + timestamps exatos |
| **0.5.5** | PATCH | Drift de letras corrigido; fila auto na primeira musica |
| **0.5.4** | PATCH | Sync de letras mais precisa; importar playlist radio/mix |
| **0.5.3** | PATCH | Letras corrigidas; modo karaoke; importar playlist YouTube |
| **0.5.2** | PATCH | Recomendados por genero; botoes com nomes simples; Descobertas |
| **0.5.1** | PATCH | Playlists por artista e misturado; generos no feed |
| **0.5.0** | MINOR | Like/dislike; algoritmo de fila personalizado por genero e gosto |
| **0.4.3** | PATCH | Atalho desktop com icone vermelho explicito |
| **0.4.1** | PATCH | Tema vermelho; instalador limpo; segurança |
| **0.4.0** | MINOR | HTML5, feed progressivo, sem mpv |
| **0.3.x** | — | Era mpv + verde + vídeo (ver CHANGELOG) |

Detalhes completos: [CHANGELOG.md](CHANGELOG.md)
