import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const $ = (id) => document.getElementById(id);

const searchInput = $("search-input");
const resultsEl = $("results");
const resultsVideoEl = $("results-video");
const recommendedEl = $("recommended");
const recommendedMusicEl = $("recommended-music");
const queueListEl = $("queue-list");
const statusEl = $("status");
const npTitle = $("np-title");
const npThumb = $("np-thumb");
const npMode = $("np-mode");
const welcomeTitle = $("welcome-title");
const welcomeText = $("welcome-text");
const welcomeList = $("welcome-list");
const mdFilename = $("md-filename");
const resultsTitle = $("results-title");
const musicShell = $("music-shell");
const videoShell = $("video-shell");
const panelWelcome = $("panel-welcome");
const panelResults = $("panel-results");
const panelResultsVideo = $("panel-results-video");
const panelRecMusic = $("panel-rec-music");
const panelRecommended = $("panel-recommended");
const panelFeed = $("panel-feed");
const panelChannelNews = $("panel-channel-news");
const panelLive = $("panel-live");
const panelPlaylist = $("panel-playlist");
const panelVideo = $("panel-video");
const videoPlayerBlock = $("video-player-block");
const videoStage = $("video-stage");
const htmlVideo = $("html-video");
const videoPlayHint = $("video-play-hint");
const videoFeedScroll = $("video-feed-scroll");
const videoHomeFeed = $("video-home-feed");
const panelVideoContext = $("panel-video-context");
const videoContextResultsEl = $("video-context-results");
const videoContextSubtitle = $("video-context-subtitle");
const btnBackHomeFeed = $("btn-back-home-feed");
const videoFeedModeLabel = $("video-feed-mode-label");
const liveResultsEl = $("live-results");
const feedResultsEl = $("feed-results");
const channelNewsResultsEl = $("channel-news-results");
const channelNewsSubtitle = $("channel-news-subtitle");
const feedSubtitle = $("feed-subtitle");
const recommendedSubtitle = $("recommended-subtitle");
const recommendedSubtitleMusic = $("recommended-subtitle-music");
const playlistResultsEl = $("playlist-results");
const playlistSubtitle = $("playlist-subtitle");
const qualityControl = $("quality-control");
const qualitySelect = $("quality-select");
const queuePanel = $("queue-panel");
const videoNpBar = $("video-np-bar");
const videoNpTitle = $("video-np-title");
const videoNpMeta = $("video-np-meta");
const rightPanelLabel = $("right-panel-label");
const queueTitle = $("queue-title");

let mode = "music";
let loggedIn = false;
let lastVideoId = null;
let currentPlaylistItems = [];
const feedCache = { music: null, video: null };
let recLoading = false;
let loadingCount = 0;
let videoFeedMode = "home";
let currentVideo = null;
let videoAutoplayPool = [];
let videoAutoplayIndex = 0;

/** Relacionados: engatilhados no play, fetch só ao rolar ou pedido explícito. */
let relatedPrime = {
  token: 0,
  video: null,
  contextDone: false,
  contextLoading: false,
  contextItems: [],
  observer: null,
};

function cancelRelatedLoads() {
  relatedPrime.token += 1;
  relatedPrime.observer?.disconnect();
  relatedPrime.observer = null;
  relatedPrime.video = null;
  relatedPrime.contextDone = false;
  relatedPrime.contextLoading = false;
  relatedPrime.contextItems = [];
  setRelatedPanelLoading(false);
}

function setRelatedPanelLoading(active) {
  panelVideoContext?.classList.toggle("related-bg-loading", active);
}

/** Busca relacionados em background — nunca bloqueia player nem overlay. */
function fetchRelatedBackground(video = currentVideo) {
  if (!video?.id || isAudioOnly()) return;
  if (relatedPrime.contextLoading) return;
  if (relatedPrime.contextItems.length && relatedPrime.video?.id === video.id) return;

  relatedPrime.contextLoading = true;
  setRelatedPanelLoading(true);
  const token = relatedPrime.token;
  const vid = video.id;

  tauriInvoke("video_context_feed", { videoId: vid })
    .then((items) => {
      if (relatedPrime.token !== token) return;
      relatedPrime.contextItems = items;
      relatedPrime.video = video;
      relatedPrime.contextDone = true;
      if (currentVideo?.id === vid) {
        buildVideoAutoplayPool(currentVideo);
      }
      if (panelVideoContext && !panelVideoContext.classList.contains("hidden")) {
        void renderCards(videoContextResultsEl, items, "sem relacionados", true);
        if (videoContextSubtitle) {
          videoContextSubtitle.textContent = `${video.title} · ${items.length} videos`;
        }
        if (videoFeedModeLabel) {
          videoFeedModeLabel.textContent = `relacionados · ${items.length}`;
        }
      }
    })
    .catch(() => {})
    .finally(() => {
      if (relatedPrime.token === token) {
        relatedPrime.contextLoading = false;
        setRelatedPanelLoading(false);
      }
    });
}

function collectVideoAutoplayCandidates() {
  const seen = new Set();
  const out = [];
  const add = (list) => {
    for (const v of list || []) {
      if (!v?.id || seen.has(v.id)) continue;
      seen.add(v.id);
      out.push(v);
    }
  };
  add(relatedPrime.contextItems);
  add(feedCache.video?.recommended);
  add(feedCache.video?.feed);
  add(feedCache.video?.channel_news);
  return out;
}

function buildVideoAutoplayPool(video) {
  videoAutoplayPool = collectVideoAutoplayCandidates();
  if (!video?.id) {
    videoAutoplayIndex = 0;
    return;
  }
  const idx = videoAutoplayPool.findIndex((v) => v.id === video.id);
  if (idx >= 0) {
    videoAutoplayIndex = idx;
  } else {
    videoAutoplayPool.unshift(video);
    videoAutoplayIndex = 0;
  }
}

async function playNextRecommendedVideo() {
  if (isAudioOnly()) {
    const v = await tauriInvoke("next");
    if (v) await play(v, false);
    return;
  }

  buildVideoAutoplayPool(currentVideo);

  if (videoAutoplayIndex + 1 < videoAutoplayPool.length) {
    videoAutoplayIndex += 1;
    await play(videoAutoplayPool[videoAutoplayIndex], true);
    setStatus("AUTO · proximo");
    void fetchRelatedBackground(videoAutoplayPool[videoAutoplayIndex]);
    return;
  }

  void fetchRelatedBackground();
  setStatus("FIM — sem proximo no feed");
}

async function playPrevRecommendedVideo() {
  if (isAudioOnly()) {
    const v = await tauriInvoke("prev");
    if (v) await play(v, false);
    return;
  }

  buildVideoAutoplayPool(currentVideo);
  if (videoAutoplayIndex > 0) {
    videoAutoplayIndex -= 1;
    await play(videoAutoplayPool[videoAutoplayIndex], true);
    setStatus("AUTO · anterior");
  }
}

function showRelatedPlaceholders(video) {
  if (videoContextSubtitle) {
    const title = video?.title ? `${video.title} · ` : "";
    videoContextSubtitle.textContent = `${title}role para carregar relacionados`;
  }
  if (videoContextResultsEl) {
    videoContextResultsEl.replaceChildren();
  }
}

function setupRelatedScrollObserver(token) {
  relatedPrime.observer?.disconnect();
  if (!videoFeedScroll) return;

  relatedPrime.observer = new IntersectionObserver(
    (entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      if (relatedPrime.token !== token) return;
      void flushRelatedContent(token);
    },
    { root: videoFeedScroll, rootMargin: "160px 0px", threshold: 0.04 }
  );

  if (panelVideoContext) relatedPrime.observer.observe(panelVideoContext);
}

async function flushContextFeed(token) {
  if (relatedPrime.contextDone || relatedPrime.contextLoading || relatedPrime.token !== token) {
    return;
  }
  const video = relatedPrime.video;
  if (!video?.id || isAudioOnly()) return;

  relatedPrime.contextLoading = true;
  setRelatedPanelLoading(true);

  try {
    const items = await tauriInvoke("video_context_feed", { videoId: video.id });
    if (relatedPrime.token !== token) return;
    relatedPrime.contextItems = items;
    relatedPrime.contextDone = true;
    await renderCards(videoContextResultsEl, items, "sem relacionados", true);
    if (videoContextSubtitle) {
      videoContextSubtitle.textContent = `${video.title} · ${items.length} videos`;
    }
    if (currentVideo?.id === video.id) {
      buildVideoAutoplayPool(currentVideo);
    }
    if (videoFeedModeLabel) {
      videoFeedModeLabel.textContent = `relacionados · ${items.length}`;
    }
  } catch (_) {
    /* silencioso — nao trava video nem status global */
  } finally {
    if (relatedPrime.token === token) {
      relatedPrime.contextLoading = false;
      setRelatedPanelLoading(false);
    }
  }
}

async function flushRelatedContent(token = relatedPrime.token) {
  if (relatedPrime.token !== token) return;
  await flushContextFeed(token);
}

/** Registra video atual + UI placeholder; não chama yt-dlp até scroll. */
function primeRelatedLoads(video) {
  if (!video?.id || isAudioOnly()) return;

  cancelRelatedLoads();
  const token = relatedPrime.token;
  relatedPrime.video = video;

  videoHomeFeed?.classList.remove("hidden");
  panelVideoContext?.classList.remove("hidden");
  panelResultsVideo?.classList.add("hidden");
  btnBackHomeFeed?.classList.add("hidden");
  if (videoFeedModeLabel) videoFeedModeLabel.textContent = "assistindo";

  showRelatedPlaceholders(video);
  setupRelatedScrollObserver(token);
}

/** Fetch manual — sempre em background, sem overlay. */
async function loadVideoContextFeed(video, { force = false } = {}) {
  if (!video?.id || isAudioOnly()) return;

  if (!force && relatedPrime.video?.id === video.id && !relatedPrime.contextDone) {
    primeRelatedLoads(video);
    return;
  }

  fetchRelatedBackground(video);
}

const LOADING_MSG = {
  search: "BUSCANDO FAIXAS...",
  rec: "CARREGANDO FEED...",
  playlist: "MONTANDO PLAYLIST...",
};

function setLoading(active, message = "CARREGANDO...", hint = "buscando no youtube via yt-dlp") {
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

const streamPrefetchIds = new Set();

function videoWatchUrl(v) {
  return v?.url || (v?.id ? `https://www.youtube.com/watch?v=${v.id}` : "");
}

function prefetchVideoStream(video) {
  if (!isTauri() || isAudioOnly() || !video?.id) return;
  if (streamPrefetchIds.has(video.id)) return;
  streamPrefetchIds.add(video.id);
  tauriInvoke("resolve_stream", {
    videoId: video.id,
    videoUrl: videoWatchUrl(video),
  }).catch(() => streamPrefetchIds.delete(video.id));
}

function prewarmUpcomingStreams() {
  if (!isTauri() || isAudioOnly() || !videoAutoplayPool.length) return;
  const upcoming = videoAutoplayPool.slice(
    videoAutoplayIndex + 1,
    videoAutoplayIndex + 3
  );
  if (!upcoming.length) return;
  tauriInvoke("prewarm_streams", { items: upcoming }).catch(() => {});
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
  const liveTag = v.is_live ? " ● LIVE" : "";
  meta.textContent = `${v.uploader || ""} · ${v.duration || ""}${liveTag}`;
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

function createYtCard(v) {
  const card = document.createElement("article");
  card.className = "yt-card" + (v.is_live ? " yt-card-live" : "");

  const thumbWrap = document.createElement("div");
  thumbWrap.className = "yt-thumb-wrap";
  const img = document.createElement("img");
  img.className = "yt-thumb";
  img.src = thumbUrl(v);
  img.alt = "";
  img.loading = "lazy";
  img.onclick = () => play(v, true);
  card.onmouseenter = () => prefetchVideoStream(v);
  if (v.duration && !v.is_live) {
    const dur = document.createElement("span");
    dur.className = "yt-duration";
    dur.textContent = v.duration;
    thumbWrap.append(img, dur);
  } else {
    thumbWrap.append(img);
  }

  const info = document.createElement("div");
  info.className = "yt-info";
  const title = document.createElement("div");
  title.className = "yt-title";
  title.title = v.title;
  title.textContent = v.title;
  title.onclick = () => play(v, true);
  const meta = document.createElement("div");
  meta.className = "yt-meta";
  meta.textContent = v.uploader || "youtube";
  const actions = document.createElement("div");
  actions.className = "yt-actions";
  const btnQueue = document.createElement("button");
  btnQueue.className = "btn-queue";
  btnQueue.textContent = "+ fila";
  btnQueue.onclick = (e) => {
    e.stopPropagation();
    enqueue(v);
  };
  actions.append(btnQueue);
  info.append(title, meta, actions);
  card.append(thumbWrap, info);
  return card;
}

async function renderCards(container, items, emptyMsg, yt = false) {
  if (!container) return;
  container.replaceChildren();
  if (!items?.length) {
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = emptyMsg;
    container.appendChild(p);
    return;
  }
  const factory = yt ? createYtCard : createCard;
  const batchSize = yt ? 6 : 4;
  for (let i = 0; i < items.length; i += batchSize) {
    const batch = items.slice(i, i + batchSize);
    for (const v of batch) {
      container.appendChild(factory(v));
    }
    if (i + batchSize < items.length) {
      await paintUi();
    }
  }
}

const LABELS = {
  music: {
    mode: "AUDIO.SYS",
    placeholder: "_buscar musica...",
    welcome: "Modo áudio — stream via mpv, sem janela de vídeo.",
    welcomeList: [
      "Digite uma query acima e pressione <code>Enter</code>",
      "Monte playlists com <code>[ REC.PL ]</code>",
      "Volume controlado na barra inferior",
    ],
    playing: "STREAM ATIVO",
    searchTitle: "Resultados da busca",
    mdFile: "README.md",
  },
  video: {
    mode: "VIDEO.SYS",
    placeholder: "_buscar video ou colar url...",
    welcome: "Modo vídeo — player integrado na página, estilo README + YouTube.",
    welcomeList: [
      "Clique no vídeo para pausar / retomar",
      "Role o feed — o player sobe junto",
      "Use <code>[ FEED INICIAL ]</code> para voltar ao explorar",
    ],
    playing: "REPRODUZINDO",
    searchTitle: "Resultados da busca",
    mdFile: "watch.md",
  },
};

function isAudioOnly() {
  return mode === "music";
}

function focusSearch() {
  searchInput?.focus();
}

function setStatus(msg) {
  if (statusEl) statusEl.textContent = msg || "";
}

function applyMode(next) {
  mode = next;
  document.body.classList.toggle("mode-video", mode === "video");
  document.querySelectorAll(".tab").forEach((t) => {
    t.classList.toggle("active", t.dataset.mode === mode);
  });
  const L = LABELS[mode];
  if (searchInput) searchInput.placeholder = L.placeholder;
  if (npMode) npMode.textContent = L.mode;
  if (welcomeTitle) welcomeTitle.textContent = mode === "video" ? "Assistir" : "Bem-vindo";
  if (welcomeText) welcomeText.textContent = L.welcome;
  if (mdFilename) mdFilename.textContent = L.mdFile;
  if (welcomeList && L.welcomeList?.length) {
    welcomeList.innerHTML = L.welcomeList.map((item) => `<li>${item}</li>`).join("");
  }
  if (resultsTitle) resultsTitle.textContent = L.searchTitle;
  if (qualityControl) qualityControl.classList.toggle("hidden", mode !== "video");
  musicShell?.classList.toggle("hidden", mode !== "music");
  videoShell?.classList.toggle("hidden", mode !== "video");
  queuePanel?.classList.toggle("hidden", mode !== "music");
  if (rightPanelLabel) rightPanelLabel.textContent = "QUEUE.SYS";
  if (queueTitle) queueTitle.textContent = "Fila";
  if (next === "music") stopVideoWeb();
  if (next === "video" && isTauri()) {
    tauriInvoke("hide_video_panel").catch(() => {});
  }
  focusSearch();
}

function showWelcome() {
  if (!isAudioOnly()) return;
  panelWelcome?.classList.remove("hidden");
  panelResults?.classList.add("hidden");
  panelRecMusic?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
}

function showMusicResults() {
  panelWelcome?.classList.add("hidden");
  panelResults?.classList.remove("hidden");
  panelRecMusic?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
}

function showVideoResults() {
  videoFeedMode = "search";
  panelResultsVideo?.classList.remove("hidden");
  videoHomeFeed?.classList.add("hidden");
  panelVideoContext?.classList.add("hidden");
  btnBackHomeFeed?.classList.remove("hidden");
  if (videoFeedModeLabel) videoFeedModeLabel.textContent = "resultados da busca";
  videoFeedScroll?.scrollTo({ top: 0, behavior: "smooth" });
}

function showVideoBrowse() {
  panelResultsVideo?.classList.add("hidden");
  if (videoFeedMode === "context") {
    videoHomeFeed?.classList.add("hidden");
    panelVideoContext?.classList.remove("hidden");
    btnBackHomeFeed?.classList.remove("hidden");
  } else {
    showHomeFeedView(false);
  }
}

function showHomeFeedView(scrollTop = true) {
  videoFeedMode = "home";
  panelResultsVideo?.classList.add("hidden");
  videoHomeFeed?.classList.remove("hidden");
  panelVideoContext?.classList.add("hidden");
  btnBackHomeFeed?.classList.add("hidden");
  if (videoFeedModeLabel) videoFeedModeLabel.textContent = "feed inicial";
  if (scrollTop) videoFeedScroll?.scrollTo({ top: 0, behavior: "smooth" });
}

function showVideoContextView(video, scrollTop = true) {
  videoFeedMode = "context";
  panelResultsVideo?.classList.add("hidden");
  videoHomeFeed?.classList.add("hidden");
  panelVideoContext?.classList.remove("hidden");
  btnBackHomeFeed?.classList.remove("hidden");
  if (videoFeedModeLabel) {
    const label = video?.title || "video atual";
    videoFeedModeLabel.textContent = `relacionados a: ${label}`;
  }
  if (scrollTop) videoFeedScroll?.scrollTo({ top: 0, behavior: "smooth" });
}

function goToHomeFeed() {
  cancelRelatedLoads();
  showHomeFeedView(true);
  if (feedCache.video) {
    renderHomeFeed(feedCache.video);
  } else {
    loadHomeFeed(true);
  }
}

function showMusicHome() {
  panelWelcome?.classList.add("hidden");
  panelResults?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
}

function showPlaylist() {
  panelWelcome?.classList.add("hidden");
  panelPlaylist?.classList.remove("hidden");
}

function showRecommendedMusic() {
  panelRecMusic?.classList.remove("hidden");
}

async function tauriInvoke(cmd, args = {}) {
  if (!isTauri()) {
    throw new Error("Abra pelo app promptub (nao pelo navegador)");
  }
  return invoke(cmd, args);
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
        "RESOLVENDO VIDEO...",
        "yt-dlp · metadata",
        async () => tauriInvoke("resolve_video", { videoId })
      );
      if (mode !== "video") applyMode("video");
      await play(video, true);
      setStatus(`URL OK — ${video.title}`);
    } catch (e) {
      setStatus(`Erro: ${e}`);
    }
    return;
  }

  try {
    const res = await withLoading(LOADING_MSG.search, "yt-dlp · ytsearch", async () =>
      tauriInvoke("search", { query: q })
    );
    if (isAudioOnly()) {
      showMusicResults();
      await renderCards(resultsEl, res, "Nenhum resultado");
    } else {
      showVideoResults();
      await renderCards(resultsVideoEl, res, "Nenhum resultado", true);
    }
    setStatus(`${res.length} HIT${res.length === 1 ? "" : "S"}`);
  } catch (e) {
    setStatus(`Erro: ${e}`);
  }
}

function updatePlayHint() {
  if (!videoPlayHint || !htmlVideo) return;
  videoPlayHint.classList.toggle("hidden", !htmlVideo.paused);
}

function toggleHtmlVideo() {
  if (!htmlVideo?.src) return;
  if (htmlVideo.paused) htmlVideo.play().catch(() => {});
  else htmlVideo.pause();
}

function stopVideoWeb() {
  if (!htmlVideo) return;
  htmlVideo.pause();
  htmlVideo.removeAttribute("src");
  htmlVideo.load();
  updatePlayHint();
}

async function playVideoWeb(video) {
  if (!htmlVideo || !video?.id) return;
  currentVideo = video;
  const vol = Number($("volume-slider")?.value ?? 100);
  setStatus("RESOLVENDO STREAM...");
  const url = await tauriInvoke("resolve_stream", {
    videoId: video.id,
    videoUrl: videoWatchUrl(video),
  });
  htmlVideo.src = url;
  htmlVideo.volume = vol / 100;
  await htmlVideo.play();
  updatePlayHint();
  prewarmUpcomingStreams();
}

async function hideVideoPanel() {
  stopVideoWeb();
  cancelRelatedLoads();
  currentVideo = null;
  videoPlayerBlock?.classList.add("hidden");
  videoNpBar?.classList.add("hidden");
  try {
    await tauriInvoke("hide_video_panel");
  } catch (_) {}
}

function initHtmlVideoPlayer() {
  if (!htmlVideo) return;
  htmlVideo.addEventListener("click", (e) => {
    e.preventDefault();
    toggleHtmlVideo();
  });
  htmlVideo.addEventListener("play", updatePlayHint);
  htmlVideo.addEventListener("pause", updatePlayHint);
  htmlVideo.addEventListener("ended", () => {
    void playNextRecommendedVideo();
  });
  document.addEventListener("keydown", (e) => {
    if (e.code !== "Space" || e.target?.matches("input, textarea, select, button")) return;
    if (isAudioOnly() || !htmlVideo?.src || videoPlayerBlock?.classList.contains("hidden")) return;
    e.preventDefault();
    toggleHtmlVideo();
  });
}

function updateVideoNowPlaying(video) {
  if (!video || isAudioOnly()) return;
  videoNpBar?.classList.remove("hidden");
  if (videoNpTitle) videoNpTitle.textContent = video.title;
  if (videoNpMeta) {
    videoNpMeta.textContent = `${video.uploader || ""}${video.duration ? ` · ${video.duration}` : ""}`;
  }
}

async function startPrewarm(items) {
  if (!items?.length) return;
  try {
    await tauriInvoke("prewarm_playlist", { items, audioOnly: isAudioOnly() });
    pollPrewarmStatus();
  } catch (_) {}
}

async function pollPrewarmStatus() {
  try {
    const s = await tauriInvoke("prewarm_status");
    if (s.total > 0 && s.done < s.total) {
      setStatus(`PREWARM ${s.done}/${s.total}`);
      setTimeout(pollPrewarmStatus, 400);
    } else if (s.total > 0 && s.done >= s.total) {
      setStatus("PREWARM OK");
    }
  } catch (_) {}
}

async function play(video, setQueue) {
  setStatus("INIT STREAM...");
  lastVideoId = video.id;
  try {
    if (!isAudioOnly()) {
      showVideoBrowse();
      videoPlayerBlock?.classList.remove("hidden");
      videoFeedScroll?.scrollTo({ top: 0 });
      updateVideoNowPlaying(video);
      buildVideoAutoplayPool(video);
      await Promise.all([
        playVideoWeb(video),
        tauriInvoke("play", { video, setQueue, audioOnly: false }),
      ]);
      primeRelatedLoads(video);
    } else {
      await hideVideoPanel();
      await tauriInvoke("play", { video, setQueue, audioOnly: true });
    }
    if (npTitle) npTitle.textContent = video.title;
    if (npThumb) {
      npThumb.src = thumbUrl(video);
      npThumb.classList.remove("hidden");
    }
    setStatus(LABELS[mode].playing);
    refreshQueue();
  } catch (e) {
    setStatus(`Erro: ${e}`);
  }
}

async function enqueue(video) {
  try {
    await tauriInvoke("enqueue", { video });
    setStatus("ENQUEUED");
    refreshQueue();
  } catch (e) {
    setStatus(`Erro: ${e}`);
  }
}

async function renderHomeFeed(feed) {
  if (isAudioOnly()) {
    await renderCards(recommendedMusicEl, feed.recommended, "sem recomendados");
    if (recommendedSubtitleMusic) {
      const n = feed.recommended?.length || 0;
      recommendedSubtitleMusic.textContent = feed.seed_label
        ? `baseado em: ${feed.seed_label} · ${n} itens`
        : `${n} itens`;
    }
    if (feed.recommended?.length) {
      showMusicHome();
      showRecommendedMusic();
    } else {
      showWelcome();
    }
    return;
  }

  showVideoBrowse();
  showHomeFeedView(false);

  if (feed.channel_news?.length) {
    await renderCards(channelNewsResultsEl, feed.channel_news, "sem novidades", true);
    if (channelNewsSubtitle) {
      const channels = [
        ...new Set(feed.channel_news.map((v) => v.uploader).filter(Boolean)),
      ];
      const label = loggedIn
        ? "inscricoes + canais que voce assiste"
        : "canais do seu historico local";
      channelNewsSubtitle.textContent =
        channels.length > 0
          ? `${label} · ${channels.slice(0, 4).join(", ")}${channels.length > 4 ? "…" : ""}`
          : label;
    }
    panelChannelNews?.classList.remove("hidden");
  } else {
    panelChannelNews?.classList.add("hidden");
  }

    if (feed.feed?.length) {
    await renderCards(feedResultsEl, feed.feed, "sem itens no feed", true);
    if (feedSubtitle) {
      const base = loggedIn
        ? "para voce · inscricoes · novidades"
        : "para voce · historico local · explorar";
      feedSubtitle.textContent = `${base} · ${feed.feed.length} videos`;
    }
    panelFeed?.classList.remove("hidden");
  } else {
    panelFeed?.classList.add("hidden");
  }

  await renderCards(recommendedEl, feed.recommended, "sem recomendados", true);
  if (recommendedSubtitle) {
    const n = feed.recommended?.length || 0;
    recommendedSubtitle.textContent = feed.seed_label
      ? `relacionados a: ${feed.seed_label} · ${n}`
      : `${n} videos para voce`;
  }
  if (feed.recommended?.length) {
    panelRecommended?.classList.remove("hidden");
  } else {
    panelRecommended?.classList.add("hidden");
  }

  if (feed.live?.length) {
    await renderCards(liveResultsEl, feed.live, "sem lives", true);
    panelLive?.classList.remove("hidden");
  } else {
    panelLive?.classList.add("hidden");
  }

  if (currentVideo && !isAudioOnly()) {
    buildVideoAutoplayPool(currentVideo);
  }
}

async function loadHomeFeed(force = false) {
  if (recLoading) return;
  if (!force && feedCache[mode]) {
    await renderHomeFeed(feedCache[mode]);
    setStatus(`FEED CACHE — ${feedCache[mode].recommended?.length || 0}`);
    return;
  }

    const btn = isAudioOnly() ? $("btn-refresh-rec-music") : $("btn-refresh-rec");
  recLoading = true;
  if (btn) btn.disabled = true;
  try {
    const feed = await withLoading(
      LOADING_MSG.rec,
      isAudioOnly() ? "mix/radio youtube" : "para voce · novidades · seu historico",
      async () => tauriInvoke("home_recommendations", { mode })
    );
    feedCache[mode] = feed;
    await renderHomeFeed(feed);
    const total =
      (feed.channel_news?.length || 0) +
      (feed.feed?.length || 0) +
      (feed.recommended?.length || 0);
    setStatus(`FEED OK — ${total}`);
  } catch (e) {
    if (isAudioOnly()) showWelcome();
    setStatus(String(e));
  } finally {
    recLoading = false;
    if (btn) btn.disabled = false;
  }
}

async function buildRecommendedPlaylist() {
  const btn = $("btn-rec-playlist");
  if (btn) btn.disabled = true;
  try {
    const res = await withLoading(
      LOADING_MSG.playlist,
      "buscas paralelas · mix + titulo",
      async () =>
        tauriInvoke("recommended_playlist", {
          seedVideoId: lastVideoId || null,
          seedQuery: searchInput?.value.trim() || null,
        })
    );
    currentPlaylistItems = res.items || [];
    if (playlistSubtitle) {
      playlistSubtitle.textContent = `Baseado em: ${res.seed_label} · ${res.count} faixas`;
    }
    await renderCards(playlistResultsEl, currentPlaylistItems, "Nenhuma faixa");
    $("btn-playlist-to-queue")?.classList.toggle("hidden", !currentPlaylistItems.length);
    showPlaylist();
    setStatus(`REC.PL OK — ${res.count} TRACKS`);
    startPrewarm(currentPlaylistItems);
  } catch (e) {
    setStatus(String(e));
  } finally {
    if (btn) btn.disabled = false;
  }
}

async function sendPlaylistToQueue() {
  if (!currentPlaylistItems.length) return;
  try {
    await tauriInvoke("load_queue", { items: currentPlaylistItems });
    setStatus(`${currentPlaylistItems.length} TRACKS → QUEUE`);
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
    setStatus("DEQUEUED");
  } catch (e) {
    setStatus(String(e));
  }
}

async function playFromQueue(index) {
  try {
    if (!isAudioOnly()) {
      videoPlayerBlock?.classList.remove("hidden");
      showVideoBrowse();
    }
    const v = await tauriInvoke("play_queue_item", { index });
    if (v) {
      lastVideoId = v.id;
      updateVideoNowPlaying(v);
      if (npTitle) npTitle.textContent = v.title;
      if (npThumb) {
        npThumb.src = thumbUrl(v);
        npThumb.classList.remove("hidden");
      }
      if (!isAudioOnly()) {
        buildVideoAutoplayPool(v);
        await playVideoWeb(v);
        primeRelatedLoads(v);
      }
      setStatus(LABELS[mode].playing);
      refreshQueue();
    }
  } catch (e) {
    setStatus(String(e));
  }
}

async function refreshQueue() {
  if (!isAudioOnly() || !queueListEl) return;
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

async function initQualitySelect() {
  if (!qualitySelect) return;
  try {
    const q = await tauriInvoke("get_video_quality");
    qualitySelect.value = q || "720";
  } catch (_) {}
}

async function init() {
  let boot = "music";
  if (isTauri()) {
    try {
      boot = await tauriInvoke("boot_mode");
    } catch (_) {}
  }
  applyMode(boot === "video" ? "video" : "music");
  focusSearch();

  if (!isTauri()) {
    showWelcome();
    setStatus("Modo preview — use o app promptub");
    return;
  }
  try {
    await tauriInvoke("check_deps");
    loggedIn = await tauriInvoke("is_logged_in");
    await initQualitySelect();
    initHtmlVideoPlayer();
    try {
      const ver = await tauriInvoke("app_version");
      if (npMode && ver) npMode.textContent = `${LABELS[mode].mode} · v${ver} · web`;
    } catch (_) {}
    listen("queue-updated", () => refreshQueue()).catch(() => {});
    listen("queue-refill", (e) => {
      refreshQueue();
      const n = typeof e.payload === "number" ? e.payload : 0;
      if (n > 0) setStatus(`FILA +${n} FAIXAS (auto)`);
    }).catch(() => {});
    const badge = $("premium-badge");
    const btnPremium = $("btn-premium");
    if (badge) badge.textContent = loggedIn ? "PREMIUM OK" : "MODO GRATUITO";
    if (btnPremium) btnPremium.textContent = loggedIn ? "[ LOGOUT ]" : "[ AUTH ] PREMIUM";
    await loadHomeFeed();
  } catch (e) {
    showWelcome();
    setStatus(String(e));
  }
}

document.querySelectorAll(".tab").forEach((btn) => {
  btn.addEventListener("click", async () => {
    applyMode(btn.dataset.mode);
    if (btn.dataset.mode === "music") await hideVideoPanel();
    if (feedCache[mode]) {
      await renderHomeFeed(feedCache[mode]);
    } else {
      await loadHomeFeed();
    }
  });
});

let resizeTimer;
btnBackHomeFeed?.addEventListener("click", () => goToHomeFeed());

$("btn-search")?.addEventListener("click", search);
$("btn-refresh-rec")?.addEventListener("click", () => loadHomeFeed(true));
$("btn-refresh-rec-music")?.addEventListener("click", () => loadHomeFeed(true));
$("btn-rec-playlist")?.addEventListener("click", buildRecommendedPlaylist);
$("btn-playlist-to-queue")?.addEventListener("click", sendPlaylistToQueue);

$("volume-slider")?.addEventListener("input", async (e) => {
  const level = Number(e.target.value);
  try {
    if (!isAudioOnly() && htmlVideo) {
      htmlVideo.volume = level / 100;
    } else {
      await tauriInvoke("set_volume", { level });
    }
  } catch (_) {}
});

qualitySelect?.addEventListener("change", async () => {
  try {
    await tauriInvoke("set_video_quality", { quality: qualitySelect.value });
    setStatus(`QUAL ${qualitySelect.value === "best" ? "MAX" : qualitySelect.value + "p"}`);
    if (!isAudioOnly() && currentVideo) {
      await playVideoWeb(currentVideo);
    }
  } catch (e) {
    setStatus(String(e));
  }
});

searchInput?.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    search();
  }
});

$("btn-stop")?.addEventListener("click", async () => {
  try {
    if (!isAudioOnly()) {
      stopVideoWeb();
      currentVideo = null;
    }
    await tauriInvoke("stop");
    await hideVideoPanel();
    setStatus("STREAM STOPPED");
    focusSearch();
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-next")?.addEventListener("click", () => {
  void playNextRecommendedVideo();
});

$("btn-prev")?.addEventListener("click", () => {
  void playPrevRecommendedVideo();
});

$("btn-clear-queue")?.addEventListener("click", async () => {
  try {
    await tauriInvoke("clear_queue");
    refreshQueue();
    setStatus("QUEUE CLEARED");
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-premium")?.addEventListener("click", async () => {
  try {
    if (loggedIn) {
      await tauriInvoke("logout");
      loggedIn = false;
      setStatus("AUTH DISCONNECTED");
    } else {
      setStatus("Abrindo login…");
      await tauriInvoke("login");
      loggedIn = true;
      setStatus("AUTH OK — PREMIUM");
    }
    const badge = $("premium-badge");
    const btnPremium = $("btn-premium");
    if (badge) badge.textContent = loggedIn ? "PREMIUM OK" : "MODO GRATUITO";
    if (btnPremium) btnPremium.textContent = loggedIn ? "[ LOGOUT ]" : "[ AUTH ] PREMIUM";
  } catch (e) {
    setStatus(`Login: ${e}`);
  }
});

init();
