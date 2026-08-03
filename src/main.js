import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";

const $ = (id) => document.getElementById(id);

const THEME_KEY = "promptub-theme";

function currentTheme() {
  return document.documentElement.getAttribute("data-theme") === "day" ? "day" : "night";
}

function applyTheme(theme) {
  const t = theme === "day" ? "day" : "night";
  document.documentElement.setAttribute("data-theme", t);
  try {
    localStorage.setItem(THEME_KEY, t);
  } catch (_) {}
  const btn = $("btn-theme");
  if (btn) btn.textContent = t === "night" ? "dia" : "noite";
  const badge = $("theme-badge");
  if (badge) badge.textContent = `audio.sys · ${t === "night" ? "noite" : "dia"}`;
  updateModeLabel();
}

function updateModeLabel(version) {
  const el = $("np-mode");
  if (!el) return;
  const ver =
    version ||
    (el.textContent.match(/v[\d.]+/) || [])[0] ||
    "";
  const t = currentTheme();
  const label = t === "night" ? "vermelho · noite" : "mono · dia";
  el.textContent = ver ? `${label} · ${ver}` : label;
}

function toggleTheme() {
  applyTheme(currentTheme() === "night" ? "day" : "night");
}

const searchInput = $("search-input");
const resultsEl = $("results");
const recommendedMusicEl = $("recommended-music");
const queueListEl = $("queue-list");
const statusEl = $("status");
const npTitle = $("np-title");
const npThumb = $("np-thumb");
const npMode = $("np-mode");
const panelWelcome = $("panel-welcome");
const panelResults = $("panel-results");
const panelFeedHome = $("panel-feed-home");
const panelContinue = $("panel-continue");
const continueEl = $("continue-music");
const panelMostPlayed = $("panel-most-played");
const panelGenrePeers = $("panel-genre-peers");
const genrePeersEl = $("genre-peers-music");
const panelNewArtists = $("panel-new-artists");
const panelHistoryMix = $("panel-history-mix");
const mostPlayedEl = $("most-played-music");
const newArtistsEl = $("new-artists-music");
const historyMixEl = $("history-mix-music");
const panelPlaylist = $("panel-playlist");
const recommendedSubtitleMusic = $("recommended-subtitle-music");
const playlistResultsEl = $("playlist-results");
const playlistSubtitle = $("playlist-subtitle");
const htmlAudio = $("html-audio");
const progressSlider = $("progress-slider");
const timeCurrent = $("time-current");
const timeTotal = $("time-total");
const volumeSlider = $("volume-slider");
const btnStop = $("btn-stop");

applyTheme(currentTheme());

let loggedIn = false;
let lastVideoId = null;
let currentVideo = null;
let currentPlaylistItems = [];
let feedCache = null;
let feedFetchId = 0;
let loadingCount = 0;
let isPlaying = false;
let progressSeeking = false;
let lyricLines = [];
let lyricLineEls = [];
let lastLyricIdx = -1;
let lyricsLoadToken = 0;
let lyricsDelayTimer = null;
let trackSwitchInProgress = false;
const streamCache = new Map();
const PREWARM_AHEAD = 3;
const VOLUME_STEP = 5;

function adjustVolume(delta) {
  if (!htmlAudio || !volumeSlider) return;
  const next = Math.min(100, Math.max(0, Number(volumeSlider.value) + delta));
  volumeSlider.value = String(next);
  htmlAudio.volume = next / 100;
  setStatus(`volume · ${next}%`);
}

const LOADING_MSG = {
  search: "BUSCANDO FAIXAS...",
  playlist: "MONTANDO PLAYLIST...",
};

function setLoading(active, message = "CARREGANDO...", hint = "resolvendo stream via yt-dlp") {
  const overlay = $("loading-overlay");
  const text = $("loading-text");
  const hintEl = $("loading-hint");
  if (!overlay) return;
  if (active) {
    loadingCount++;
    if (text) text.textContent = message;
    if (hintEl) hintEl.textContent = hint;
    overlay.classList.remove("hidden");
    overlay.setAttribute("aria-busy", "true");
    document.body.classList.add("is-loading");
  } else {
    loadingCount = Math.max(0, loadingCount - 1);
    if (loadingCount === 0) {
      overlay.classList.add("hidden");
      overlay.setAttribute("aria-busy", "false");
      document.body.classList.remove("is-loading");
    }
  }
}

async function paintUi() {
  await new Promise((r) => requestAnimationFrame(r));
}

async function withLoading(message, hint, fn) {
  setLoading(true, message, hint);
  await paintUi();
  try {
    return await fn();
  } finally {
    setLoading(false);
  }
}

function thumbUrl(v) {
  return v.thumbnail || `https://i.ytimg.com/vi/${v.id}/hqdefault.jpg`;
}

function formatTime(sec) {
  if (!Number.isFinite(sec) || sec < 0) return "0:00";
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function updatePlayButton() {
  if (!btnStop) return;
  btnStop.textContent = isPlaying ? "⏸" : "▶";
  btnStop.title = isPlaying ? "Pausar" : "Tocar";
}

function updateNowPlaying(video) {
  lastVideoId = video.id;
  currentVideo = video;
  if (npTitle) npTitle.textContent = video.title;
  if (npThumb) {
    npThumb.src = thumbUrl(video);
    npThumb.classList.remove("hidden");
  }
  scheduleLyricsLoad(video);
  void refreshTasteButtons();
}

function setTasteUi(state) {
  const btnLike = $("btn-like");
  const btnDislike = $("btn-dislike");
  btnLike?.classList.toggle("active-like", state === "liked");
  btnDislike?.classList.toggle("active-dislike", state === "disliked");
  btnLike?.setAttribute("aria-pressed", state === "liked" ? "true" : "false");
  btnDislike?.setAttribute("aria-pressed", state === "disliked" ? "true" : "false");
}

async function refreshTasteButtons() {
  if (!currentVideo) {
    setTasteUi("none");
    return;
  }
  try {
    const status = await tauriInvoke("taste_get", { video: currentVideo });
    setTasteUi(status?.state || "none");
  } catch (_) {
    setTasteUi("none");
  }
}

function scheduleLyricsLoad(video) {
  if (lyricsDelayTimer) clearTimeout(lyricsDelayTimer);
  const scroll = $("lyrics-scroll");
  const trackLabel = $("lyrics-track");
  if (trackLabel) trackLabel.textContent = video.title;
  if (scroll) {
    scroll.replaceChildren();
    const p = document.createElement("p");
    p.className = "lyrics-placeholder muted";
    p.textContent = "letra em breve…";
    scroll.appendChild(p);
  }
  lyricsDelayTimer = setTimeout(() => {
    void loadLyrics(video);
  }, 2000);
}

async function loadLyrics(video) {
  const scroll = $("lyrics-scroll");
  const trackLabel = $("lyrics-track");
  if (!scroll) return;

  const token = ++lyricsLoadToken;
  lyricLines = [];
  lyricLineEls = [];
  lastLyricIdx = -1;

  if (trackLabel) trackLabel.textContent = video.title;
  scroll.replaceChildren();
  const loading = document.createElement("p");
  loading.className = "lyrics-placeholder muted";
  loading.textContent = "buscando letra...";
  scroll.appendChild(loading);

  try {
    const lines = await tauriInvoke("fetch_lyrics", {
      videoId: video.id,
      title: video.title || "",
      artist: video.uploader || "",
    });
    if (token !== lyricsLoadToken) return;
    renderLyrics(lines);
  } catch {
    if (token !== lyricsLoadToken) return;
    scroll.replaceChildren();
    const p = document.createElement("p");
    p.className = "lyrics-placeholder muted";
    p.textContent = "sem letra sincronizada";
    scroll.appendChild(p);
  }
}

function renderLyrics(lines) {
  const scroll = $("lyrics-scroll");
  if (!scroll || !Array.isArray(lines) || !lines.length) return;

  scroll.replaceChildren();
  lyricLines = lines;
  lyricLineEls = lines.map((line, i) => {
    const p = document.createElement("p");
    p.className = "lyrics-line";
    p.dataset.index = String(i);
    p.textContent = line.text;
    p.title = "Ir para este trecho";
    p.onclick = () => {
      if (htmlAudio && Number.isFinite(line.start)) {
        htmlAudio.currentTime = line.start;
      }
    };
    scroll.appendChild(p);
    return p;
  });
}

function syncLyricsHighlight() {
  if (!htmlAudio || !lyricLines.length || !lyricLineEls.length) return;

  const t = htmlAudio.currentTime;
  let activeIdx = -1;
  for (let i = 0; i < lyricLines.length; i++) {
    const line = lyricLines[i];
    if (t >= line.start && t < line.end) {
      activeIdx = i;
      break;
    }
  }
  if (activeIdx < 0) {
    for (let i = lyricLines.length - 1; i >= 0; i--) {
      if (t >= lyricLines[i].start) {
        activeIdx = i;
        break;
      }
    }
  }
  if (activeIdx === lastLyricIdx) return;

  lastLyricIdx = activeIdx;
  lyricLineEls.forEach((el, i) => {
    el.classList.toggle("active", i === activeIdx);
  });
  if (activeIdx >= 0 && lyricLineEls[activeIdx]) {
    lyricLineEls[activeIdx].scrollIntoView({ block: "center", behavior: "smooth" });
  }
}

function focusSearch() {
  searchInput?.focus();
}

function setStatus(msg) {
  if (statusEl) statusEl.textContent = msg || "";
}

function showWelcome() {
  panelWelcome?.classList.remove("hidden");
  panelResults?.classList.add("hidden");
  panelFeedHome?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
}

function setMdPath(...segments) {
  const mdPath = $("md-filepath");
  if (!mdPath) return;
  mdPath.replaceChildren();
  segments.forEach((seg, i) => {
    if (i > 0) mdPath.appendChild(document.createTextNode(" / "));
    const code = document.createElement("code");
    code.textContent = seg;
    mdPath.appendChild(code);
  });
}

function showMusicResults() {
  panelWelcome?.classList.add("hidden");
  panelResults?.classList.remove("hidden");
  panelFeedHome?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
  $("btn-nav-home")?.classList.remove("active");
  const q = searchInput?.value.trim();
  if (q) setMdPath("promptub", "busca", q);
}

function showMusicHome() {
  panelWelcome?.classList.add("hidden");
  panelResults?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
  panelFeedHome?.classList.remove("hidden");
}

async function tauriInvoke(cmd, args = {}) {
  if (!isTauri()) {
    throw new Error("Abra pelo app promptub (nao pelo navegador)");
  }
  return invoke(cmd, args);
}

function showPlaylist() {
  panelWelcome?.classList.add("hidden");
  panelFeedHome?.classList.add("hidden");
  panelPlaylist?.classList.remove("hidden");
}

async function resolveStreamUrl(video) {
  if (streamCache.has(video.id)) return streamCache.get(video.id);
  const url = await tauriInvoke("resolve_stream", {
    videoId: video.id,
    videoUrl: video.url || null,
  });
  streamCache.set(video.id, url);
  return url;
}

async function prewarmNextInQueue() {
  try {
    const q = await tauriInvoke("get_queue");
    const upcoming = q.items.slice(q.current + 1, q.current + 1 + PREWARM_AHEAD);
    if (!upcoming.length) return;
    await tauriInvoke("prewarm_playlist", { items: upcoming, audioOnly: true });
    for (const v of upcoming) {
      if (!streamCache.has(v.id)) {
        resolveStreamUrl(v).catch(() => {});
      }
    }
  } catch (_) {}
}

async function streamAndPlay(video) {
  if (!htmlAudio) throw new Error("Player de audio indisponivel");
  updateNowPlaying(video);
  setStatus("resolvendo stream...");
  const url = await resolveStreamUrl(video);
  htmlAudio.src = url;
  htmlAudio.volume = Number(volumeSlider?.value ?? 100) / 100;
  await htmlAudio.play();
  isPlaying = true;
  updatePlayButton();
  setStatus("tocando · web");
  refreshQueue();
  void prewarmNextInQueue();
}

async function play(video, setQueue) {
  try {
    await tauriInvoke("play", { video, setQueue, audioOnly: true });
    await streamAndPlay(video);
  } catch (e) {
    isPlaying = false;
    updatePlayButton();
    setStatus(`Erro: ${e}`);
  }
}

function createCard(v) {
  const card = document.createElement("article");
  card.className = "card" + (v.is_live ? " card-live" : "");
  const img = document.createElement("img");
  img.src = thumbUrl(v);
  img.alt = "";
  img.loading = "lazy";

  const info = document.createElement("div");
  info.className = "card-info";
  const title = document.createElement("div");
  title.className = "card-title";
  title.title = v.title;
  title.textContent = v.title;
  const meta = document.createElement("div");
  meta.className = "card-meta";
  meta.textContent = `${v.uploader || ""} · ${v.duration || ""}`;
  info.append(title, meta);

  const actions = document.createElement("div");
  actions.className = "card-actions";
  const btnPlay = document.createElement("button");
  btnPlay.className = "btn-play";
  btnPlay.textContent = "[PLAY]";
  btnPlay.onclick = () => play(v, true);
  const btnQueue = document.createElement("button");
  btnQueue.className = "btn-queue";
  btnQueue.textContent = "[+Q]";
  btnQueue.onclick = () => enqueue(v);
  actions.append(btnPlay, btnQueue);

  card.append(img, info, actions);
  return card;
}

async function renderCards(container, items, emptyMsg) {
  if (!container) return;
  container.replaceChildren();
  if (!items?.length) {
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = emptyMsg;
    container.appendChild(p);
    return;
  }
  for (let i = 0; i < items.length; i += 4) {
    for (const v of items.slice(i, i + 4)) {
      container.appendChild(createCard(v));
    }
    if (i + 4 < items.length) await paintUi();
  }
}

function extractYoutubeId(input) {
  const s = input.trim();
  if (/^[a-zA-Z0-9_-]{11}$/.test(s)) return s;
  const patterns = [
    /(?:youtube\.com\/watch\?.*v=|youtu\.be\/|youtube\.com\/embed\/|youtube\.com\/live\/|youtube\.com\/shorts\/)([a-zA-Z0-9_-]{11})/,
    /[?&]v=([a-zA-Z0-9_-]{11})/,
  ];
  for (const re of patterns) {
    const m = s.match(re);
    if (m) return m[1];
  }
  return null;
}

async function search() {
  if (!searchInput) return;
  const q = searchInput.value.trim();
  if (!q) return;

  const videoId = extractYoutubeId(q);
  if (videoId) {
    try {
      const video = await withLoading(
        "RESOLVENDO FAIXA...",
        "yt-dlp · metadata",
        async () => tauriInvoke("resolve_video", { videoId })
      );
      await play(video, true);
    } catch (e) {
      setStatus(`Erro: ${e}`);
    }
    return;
  }

  try {
    const res = await withLoading(LOADING_MSG.search, "yt-dlp · ytsearch", async () =>
      tauriInvoke("search", { query: q })
    );
    showMusicResults();
    await renderCards(resultsEl, res, "Nenhum resultado");
    setStatus(`${res.length} hit${res.length === 1 ? "" : "s"}`);
    setMdPath("promptub", "busca", q);
  } catch (e) {
    setStatus(`Erro: ${e}`);
  }
}

async function startPrewarm(items) {
  if (!items?.length) return;
  try {
    await tauriInvoke("prewarm_playlist", { items, audioOnly: true });
    pollPrewarmStatus();
  } catch (_) {}
}

async function pollPrewarmStatus() {
  try {
    const s = await tauriInvoke("prewarm_status");
    if (s.total > 0 && s.done < s.total) {
      setStatus(`prewarm ${s.done}/${s.total}`);
      setTimeout(pollPrewarmStatus, 400);
    } else if (s.total > 0 && s.done >= s.total) {
      setStatus("streams prontos");
    }
  } catch (_) {}
}

async function enqueue(video) {
  try {
    await tauriInvoke("enqueue", { video });
    setStatus("enqueued");
    refreshQueue();
  } catch (e) {
    setStatus(`Erro: ${e}`);
  }
}

async function renderFeedRow(container, items, emptyMsg) {
  if (!container) return;
  container.replaceChildren();
  if (!items?.length) {
    const p = document.createElement("p");
    p.className = "muted feed-empty";
    p.textContent = emptyMsg;
    container.appendChild(p);
    return;
  }
  for (const v of items) {
    container.appendChild(createCard(v));
  }
}

async function playRow(items) {
  if (!items?.length) return;
  await tauriInvoke("load_queue", { items });
  await play(items[0], false);
}

async function queueRow(items) {
  if (!items?.length) return;
  for (const v of items) await tauriInvoke("enqueue", { video: v });
  setStatus(`+${items.length} na fila`);
  refreshQueue();
  void prewarmNextInQueue();
}

function emptyFeed() {
  return {
    recommended: [],
    continue_listening: [],
    most_played: [],
    new_artists: [],
    history_mix: [],
    genre_rows: [],
    feed: [],
    seed_label: "",
  };
}

function collectFeedIds() {
  if (!feedCache) return [];
  const ids = new Set();
  for (const key of [
    "continue_listening",
    "recommended",
    "most_played",
    "feed",
    "new_artists",
    "history_mix",
  ]) {
    for (const v of feedCache[key] || []) {
      if (v?.id) ids.add(v.id);
    }
  }
  for (const row of feedCache.genre_rows || []) {
    for (const v of row.items || []) {
      if (v?.id) ids.add(v.id);
    }
  }
  return [...ids];
}

async function renderGenreRowsMount(rows) {
  const mount = $("genre-rows-mount");
  if (!mount) return;
  mount.replaceChildren();
  if (!rows?.length) return;

  for (const row of rows) {
    const section = document.createElement("section");
    section.className = "feed-section";

    const header = document.createElement("div");
    header.className = "section-header";

    const info = document.createElement("div");
    const h2 = document.createElement("h2");
    h2.textContent = row.label;
    const sub = document.createElement("p");
    sub.className = "muted";
    sub.textContent = `${row.items?.length || 0} faixas · seu gosto`;
    info.append(h2, sub);

    const btnPlay = document.createElement("button");
    btnPlay.className = "btn-accent";
    btnPlay.textContent = "[ PLAY ]";
    btnPlay.onclick = () => playRow(row.items);

    const btnQueue = document.createElement("button");
    btnQueue.className = "btn-accent";
    btnQueue.textContent = "[ +FILA ]";
    btnQueue.onclick = () => queueRow(row.items);

    header.append(info, btnPlay, btnQueue);
    section.append(header);

    const rowEl = document.createElement("div");
    rowEl.className = "feed-row";
    await renderFeedRow(rowEl, row.items || [], "");
    section.append(rowEl);

    mount.append(section);
    void prewarmFeedItems(row.items);
  }
}

function setSectionLoading(panelId, loading) {
  const panel = $(panelId);
  if (!panel) return;
  panel.classList.toggle("section-loading", loading);
}

async function prewarmFeedItems(items) {
  const batch = (items || []).slice(0, 3);
  if (!batch.length) return;
  try {
    await tauriInvoke("prewarm_playlist", { items: batch, audioOnly: true });
    for (const v of batch) {
      if (!streamCache.has(v.id)) resolveStreamUrl(v).catch(() => {});
    }
  } catch (_) {}
}

async function renderLocalFeedParts(local) {
  if (!feedCache) feedCache = emptyFeed();
  feedCache.continue_listening = local.continue_listening || [];
  feedCache.most_played = local.most_played || [];
  feedCache.seed_label = local.seed_label || feedCache.seed_label || "";

  const hasContinue = feedCache.continue_listening.length > 0;
  panelContinue?.classList.toggle("hidden", !hasContinue);
  if (hasContinue) {
    await renderFeedRow(continueEl, feedCache.continue_listening, "");
    const lastTitle = feedCache.continue_listening[0]?.title || "";
    $("continue-subtitle").textContent = lastTitle
      ? `ultima · ${lastTitle.slice(0, 48)}${lastTitle.length > 48 ? "…" : ""}`
      : `${feedCache.continue_listening.length} faixas recentes`;
    void prewarmFeedItems(feedCache.continue_listening);
  }

  const hasMost = feedCache.most_played.length > 0;
  panelMostPlayed?.classList.toggle("hidden", !hasMost);
  if (hasMost) {
    await renderFeedRow(mostPlayedEl, feedCache.most_played, "");
    $("most-played-subtitle").textContent = `${feedCache.most_played.length} do seu historico`;
  }
}

async function applyFeedSection(section, items) {
  if (!feedCache) feedCache = emptyFeed();
  const map = {
    recommended: ["recommended", recommendedMusicEl, "recommended-subtitle-music"],
    peers: ["feed", genrePeersEl, "genre-peers-subtitle"],
    new_artists: ["new_artists", newArtistsEl, "new-artists-subtitle"],
    history_mix: ["history_mix", historyMixEl, "history-mix-subtitle"],
  };
  const cfg = map[section];
  if (!cfg) return;
  const [field, el, subtitleId] = cfg;
  feedCache[field] = items || [];

  if (section === "recommended") {
    await renderFeedRow(el, items, "sem recomendados");
    if (recommendedSubtitleMusic) {
      recommendedSubtitleMusic.textContent = feedCache.seed_label
        ? `${feedCache.seed_label} · ${items.length} faixas`
        : `${items.length} faixas`;
    }
    void prewarmFeedItems(items);
    return;
  }

  const panel =
    section === "peers"
      ? panelGenrePeers
      : section === "new_artists"
        ? panelNewArtists
        : panelHistoryMix;
  panel?.classList.toggle("hidden", !items?.length);
  if (items?.length) {
    await renderFeedRow(el, items, "");
    const sub = $(subtitleId);
    if (sub) {
      const labels = {
        peers: `${items.length} artistas do mesmo genero`,
        new_artists: `${items.length} novidades do seu genero`,
        history_mix: `feito pra voce · ${items.length} faixas`,
      };
      sub.textContent = labels[section] || `${items.length} faixas`;
    }
    void prewarmFeedItems(items);
  }
}

async function loadLazyFeedSections(fetchId, essentialOnly = false) {
  if (essentialOnly) return;
  const sections = [
    { key: "peers", panel: "panel-genre-peers" },
    { key: "new_artists", panel: "panel-new-artists" },
    { key: "history_mix", panel: "panel-history-mix" },
  ];

  await Promise.all([
    ...sections.map(async ({ key, panel }) => {
      setSectionLoading(panel, true);
      try {
        const res = await tauriInvoke("home_feed_section", {
          section: key,
          excludeIds: collectFeedIds(),
        });
        if (fetchId !== feedFetchId) return;
        await applyFeedSection(key, res.items || []);
      } catch (_) {}
      setSectionLoading(panel, false);
    }),
    (async () => {
      try {
        const res = await tauriInvoke("home_feed_genres", {
          excludeIds: collectFeedIds(),
        });
        if (fetchId !== feedFetchId) return;
        if (!feedCache) feedCache = emptyFeed();
        feedCache.genre_rows = res.rows || [];
        await renderGenreRowsMount(feedCache.genre_rows);
      } catch (_) {}
    })(),
  ]);

  if (fetchId === feedFetchId && feedCache) {
    try {
      await tauriInvoke("save_stored_feed", { feed: feedCache });
    } catch (_) {}
  }
}

async function loadHomeFeedProgressive({ force = false, essentialOnly = false } = {}) {
  if (!force && feedCache?.recommended?.length) {
    await renderHomeFeed(feedCache);
    return;
  }

  const fetchId = ++feedFetchId;
  const btn = $("btn-refresh-rec-music");
  if (btn) btn.disabled = true;

  try {
    const local = await tauriInvoke("home_feed_local");
    if (fetchId !== feedFetchId) return;
    if (!feedCache) feedCache = emptyFeed();
    await renderLocalFeedParts(local);
    showMusicHome();
    setStatus(`feed · ${local.seed_label || "carregando"}`);

    setSectionLoading("panel-rec-music", true);
    const rec = await tauriInvoke("home_feed_section", {
      section: "recommended",
      excludeIds: collectFeedIds(),
      essential: essentialOnly,
    });
    if (fetchId !== feedFetchId) return;
    await applyFeedSection("recommended", rec.items || []);
    setSectionLoading("panel-rec-music", false);
    setStatus(`${rec.items?.length || 0} recomendados · ${feedCache.seed_label || "feed"}`);

    void loadLazyFeedSections(fetchId, essentialOnly);
  } catch (e) {
    if (fetchId !== feedFetchId) return;
    if (!feedCache?.recommended?.length) showWelcome();
    setStatus(String(e));
  } finally {
    if (fetchId === feedFetchId && btn) btn.disabled = false;
  }
}

async function renderHomeFeed(feed) {
  feedCache = feed;
  showMusicHome();
  await renderLocalFeedParts({
    continue_listening: feed.continue_listening,
    most_played: feed.most_played,
    seed_label: feed.seed_label,
  });
  await applyFeedSection("recommended", feed.recommended || []);
  await applyFeedSection("peers", feed.feed || []);
  await applyFeedSection("new_artists", feed.new_artists || []);
  await applyFeedSection("history_mix", feed.history_mix || []);
  await renderGenreRowsMount(feed.genre_rows || []);

  const genreCount = (feed.genre_rows || []).reduce((n, r) => n + (r.items?.length || 0), 0);
  const any =
    (feed.continue_listening?.length || 0) +
      (feed.recommended?.length || 0) +
      (feed.feed?.length || 0) +
      (feed.most_played?.length || 0) +
      (feed.new_artists?.length || 0) +
      (feed.history_mix?.length || 0) +
      genreCount >
    0;
  if (any) showMusicHome();
  else showWelcome();
}

async function goHomeFeed(refresh = false) {
  $("btn-nav-home")?.classList.add("active");
  panelResults?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
  document.querySelector(".main-inner")?.scrollTo({ top: 0, behavior: "smooth" });
  setMdPath("promptub", "feed", "inicio");

  showMusicHome();

  if (feedCache) {
    await renderHomeFeed(feedCache);
    setStatus(refresh ? `atualizando · ${feedCache.seed_label || "feed"}` : `feed · ${feedCache.recommended?.length || 0} faixas`);
  }

  void loadHomeFeedProgressive({ force: true, essentialOnly: refresh });
}

async function buildPlaylist(mode = "personal", title = "Playlist recomendada") {
  const btnIds = {
    personal: "btn-rec-playlist",
    artist: "btn-artist-playlist",
    mixed: "btn-mixed-playlist",
    discoveries: "btn-discoveries-playlist",
  };
  const btn = $(btnIds[mode] || "btn-rec-playlist");
  if (btn) btn.disabled = true;
  try {
    const res = await withLoading(
      LOADING_MSG.playlist,
      mode === "artist" ? "buscando faixas do artista" : "buscas paralelas",
      async () =>
        tauriInvoke("recommended_playlist", {
          seedVideoId: lastVideoId || null,
          seedQuery: searchInput?.value.trim() || null,
          mode,
        })
    );
    currentPlaylistItems = res.items || [];
    if ($("playlist-title")) $("playlist-title").textContent = title;
    if (playlistSubtitle) {
      playlistSubtitle.textContent = `${res.seed_label} · ${res.count} faixas`;
    }
    await renderCards(playlistResultsEl, currentPlaylistItems, "Nenhuma faixa");
    $("btn-playlist-to-queue")?.classList.toggle("hidden", !currentPlaylistItems.length);
    showPlaylist();
    setStatus(`${mode} · ${res.count} tracks`);
    startPrewarm(currentPlaylistItems);
  } catch (e) {
    setStatus(String(e));
  } finally {
    if (btn) btn.disabled = false;
  }
}

async function buildRecommendedPlaylist() {
  return buildPlaylist("personal", "Pra você");
}

async function sendPlaylistToQueue() {
  if (!currentPlaylistItems.length) return;
  try {
    await tauriInvoke("load_queue", { items: currentPlaylistItems });
    setStatus(`${currentPlaylistItems.length} tracks → queue`);
    startPrewarm(currentPlaylistItems);
    refreshQueue();
  } catch (e) {
    setStatus(String(e));
  }
}

async function removeFromQueue(index) {
  try {
    await tauriInvoke("remove_queue_item", { index });
    refreshQueue();
    setStatus("dequeued");
  } catch (e) {
    setStatus(String(e));
  }
}

async function playFromQueue(index) {
  try {
    const v = await tauriInvoke("play_queue_item", { index });
    if (v) await streamAndPlay(v);
  } catch (e) {
    setStatus(String(e));
  }
}

async function switchTrack(direction) {
  if (trackSwitchInProgress) return;
  trackSwitchInProgress = true;
  try {
    const v = await tauriInvoke(direction === "next" ? "next" : "prev");
    if (v) await streamAndPlay(v);
    else if (direction === "next") {
      isPlaying = false;
      updatePlayButton();
      setStatus("fim da fila");
    }
  } catch (e) {
    setStatus(String(e));
  } finally {
    trackSwitchInProgress = false;
  }
}

async function playNextAuto() {
  await switchTrack("next");
}

async function refreshQueue() {
  if (!queueListEl) return;
  try {
    const q = await tauriInvoke("get_queue");
    queueListEl.replaceChildren();
    q.items.forEach((item, i) => {
      const li = document.createElement("li");
      li.className = "queue-item";
      if (i === q.current) li.classList.add("playing");

      const thumb = document.createElement("img");
      thumb.className = "queue-item-thumb";
      thumb.src = thumbUrl(item);
      thumb.alt = "";
      thumb.loading = "lazy";
      thumb.onclick = () => playFromQueue(i);

      const body = document.createElement("div");
      body.className = "queue-item-body";
      const title = document.createElement("span");
      title.className = "queue-item-title";
      title.textContent = item.title;
      title.title = item.title;
      title.onclick = () => playFromQueue(i);
      const meta = document.createElement("span");
      meta.className = "queue-item-meta";
      meta.textContent = `${item.uploader || ""}${item.duration ? ` · ${item.duration}` : ""}`;
      body.append(title, meta);

      const btnRemove = document.createElement("button");
      btnRemove.className = "btn-queue-remove";
      btnRemove.type = "button";
      btnRemove.title = "Remover da fila";
      btnRemove.textContent = "×";
      btnRemove.onclick = (e) => {
        e.stopPropagation();
        removeFromQueue(i);
      };

      li.append(thumb, body, btnRemove);
      queueListEl.appendChild(li);
    });
  } catch (_) {}
}

function setupAudioPlayer() {
  if (!htmlAudio) return;

  htmlAudio.addEventListener("play", () => {
    isPlaying = true;
    updatePlayButton();
  });

  htmlAudio.addEventListener("pause", () => {
    isPlaying = false;
    updatePlayButton();
  });

  htmlAudio.addEventListener("ended", () => {
    void playNextAuto();
  });

  htmlAudio.addEventListener("timeupdate", () => {
    if (progressSeeking || !htmlAudio.duration) return;
    if (progressSlider) {
      progressSlider.value = String((htmlAudio.currentTime / htmlAudio.duration) * 100);
    }
    if (timeCurrent) timeCurrent.textContent = formatTime(htmlAudio.currentTime);
    if (timeTotal) timeTotal.textContent = formatTime(htmlAudio.duration);
    syncLyricsHighlight();
  });

  htmlAudio.addEventListener("loadedmetadata", () => {
    if (timeTotal) timeTotal.textContent = formatTime(htmlAudio.duration);
  });

  htmlAudio.addEventListener("error", () => {
    isPlaying = false;
    updatePlayButton();
    setStatus("Erro ao tocar stream — tente outra faixa");
  });

  progressSlider?.addEventListener("mousedown", () => {
    progressSeeking = true;
  });
  progressSlider?.addEventListener("touchstart", () => {
    progressSeeking = true;
  });
  progressSlider?.addEventListener("input", () => {
    if (!htmlAudio.duration) return;
    const pct = Number(progressSlider.value) / 100;
    htmlAudio.currentTime = pct * htmlAudio.duration;
    if (timeCurrent) timeCurrent.textContent = formatTime(htmlAudio.currentTime);
  });
  progressSlider?.addEventListener("change", () => {
    progressSeeking = false;
  });

  volumeSlider?.addEventListener("input", (e) => {
    if (htmlAudio) htmlAudio.volume = Number(e.target.value) / 100;
  });
}

async function init() {
  setupAudioPlayer();
  focusSearch();

  if (!isTauri()) {
    showWelcome();
    setStatus("modo preview — use o app promptub");
    return;
  }

  showMusicHome();
  setStatus("carregando feed…");

  try {
    await tauriInvoke("check_deps");
    loggedIn = await tauriInvoke("is_logged_in");
    try {
      const ver = await tauriInvoke("app_version");
      if (ver) updateModeLabel(`v${ver}`);
    } catch (_) {}
    listen("queue-updated", () => {
      refreshQueue();
      void prewarmNextInQueue();
    }).catch(() => {});
    listen("queue-refill", (e) => {
      refreshQueue();
      const n = typeof e.payload === "number" ? e.payload : 0;
      if (n > 0) setStatus(`fila +${n} faixas`);
    }).catch(() => {});
    const badge = $("premium-badge");
    const btnPremium = $("btn-premium");
    if (badge) badge.textContent = loggedIn ? "premium ok" : "modo gratuito";
    if (btnPremium) btnPremium.textContent = loggedIn ? "[ LOGOUT ]" : "[ AUTH ] premium";

    try {
      const stored = await tauriInvoke("get_stored_feed");
      if (stored) {
        feedCache = stored;
        await renderHomeFeed(stored);
        setStatus(`feed salvo · ${stored.seed_label || "historico"}`);
      }
    } catch (_) {}

    await loadHomeFeedProgressive({ force: true, essentialOnly: false });
  } catch (e) {
    showWelcome();
    setStatus(String(e));
  }
}

$("btn-logo-home")?.addEventListener("click", () => goHomeFeed(true));

$("btn-theme")?.addEventListener("click", () => toggleTheme());

$("btn-nav-home")?.addEventListener("click", () => goHomeFeed(false));

$("btn-play-rec-row")?.addEventListener("click", () => playRow(feedCache?.recommended));
$("btn-queue-rec-row")?.addEventListener("click", () => queueRow(feedCache?.recommended));
$("btn-play-continue-row")?.addEventListener("click", () => playRow(feedCache?.continue_listening));
$("btn-queue-continue-row")?.addEventListener("click", () => queueRow(feedCache?.continue_listening));
$("btn-play-peers-row")?.addEventListener("click", () => playRow(feedCache?.feed));
$("btn-queue-peers-row")?.addEventListener("click", () => queueRow(feedCache?.feed));
$("btn-play-most-row")?.addEventListener("click", () => playRow(feedCache?.most_played));
$("btn-queue-most-row")?.addEventListener("click", () => queueRow(feedCache?.most_played));
$("btn-play-new-row")?.addEventListener("click", () => playRow(feedCache?.new_artists));
$("btn-queue-new-row")?.addEventListener("click", () => queueRow(feedCache?.new_artists));
$("btn-play-history-row")?.addEventListener("click", () => playRow(feedCache?.history_mix));
$("btn-queue-history-row")?.addEventListener("click", () => queueRow(feedCache?.history_mix));
$("btn-open-history-pl")?.addEventListener("click", buildRecommendedPlaylist);

document.addEventListener("keydown", (e) => {
  const tag = e.target?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA") {
    if (e.key === "Escape") e.target.blur();
    return;
  }
  if (e.code === "Space") {
    e.preventDefault();
    btnStop?.click();
  } else if (e.key === "ArrowRight" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    $("btn-next")?.click();
  } else if (e.key === "ArrowLeft" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    $("btn-prev")?.click();
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    adjustVolume(VOLUME_STEP);
  } else if (e.key === "ArrowDown") {
    e.preventDefault();
    adjustVolume(-VOLUME_STEP);
  }
});

$("btn-refresh-rec-music")?.addEventListener("click", () => {
  if (feedCache) {
    void renderHomeFeed(feedCache);
    setStatus(`atualizando · ${feedCache.seed_label || "feed"}`);
  }
  void loadHomeFeedProgressive({ force: true, essentialOnly: false });
});
$("btn-rec-playlist")?.addEventListener("click", buildRecommendedPlaylist);
$("btn-artist-playlist")?.addEventListener("click", () =>
  buildPlaylist("artist", "Do artista")
);
$("btn-mixed-playlist")?.addEventListener("click", () =>
  buildPlaylist("mixed", "Misturado")
);
$("btn-discoveries-playlist")?.addEventListener("click", () =>
  buildPlaylist("discoveries", "Novidades")
);
$("btn-playlist-to-queue")?.addEventListener("click", sendPlaylistToQueue);

searchInput?.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    search();
  }
});

$("btn-search")?.addEventListener("click", () => search());

btnStop?.addEventListener("click", async () => {
  if (!htmlAudio) return;
  if (isPlaying) {
    htmlAudio.pause();
    setStatus("pausado");
  } else if (htmlAudio.src) {
    try {
      await htmlAudio.play();
      setStatus("tocando · web");
    } catch (e) {
      setStatus(String(e));
    }
  } else {
    focusSearch();
  }
});

$("btn-next")?.addEventListener("click", () => void switchTrack("next"));

$("btn-prev")?.addEventListener("click", () => void switchTrack("prev"));

$("btn-like")?.addEventListener("click", async () => {
  if (!currentVideo) return;
  try {
    const status = await tauriInvoke("taste_like", { video: currentVideo });
    setTasteUi(status?.state || "liked");
    setStatus("gostei · fila personalizada");
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-dislike")?.addEventListener("click", async () => {
  if (!currentVideo) return;
  try {
    const status = await tauriInvoke("taste_dislike", { video: currentVideo });
    setTasteUi(status?.state || "disliked");
    setStatus("nao gostei · removido da fila");
    await refreshQueue();
    await switchTrack("next");
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-clear-queue")?.addEventListener("click", async () => {
  try {
    await tauriInvoke("clear_queue");
    refreshQueue();
    setStatus("queue cleared");
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-premium")?.addEventListener("click", async () => {
  try {
    if (loggedIn) {
      await tauriInvoke("logout");
      loggedIn = false;
      setStatus("auth disconnected");
    } else {
      setStatus("abrindo login…");
      await tauriInvoke("login");
      loggedIn = true;
      setStatus("auth ok · premium");
    }
    const badge = $("premium-badge");
    const btnPremium = $("btn-premium");
    if (badge) badge.textContent = loggedIn ? "premium ok" : "modo gratuito";
    if (btnPremium) btnPremium.textContent = loggedIn ? "[ LOGOUT ]" : "[ AUTH ] premium";
  } catch (e) {
    setStatus(`Login: ${e}`);
  }
});

init();
