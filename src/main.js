import { invoke, isTauri } from "@tauri-apps/api/core";

const $ = (id) => document.getElementById(id);

const searchInput = $("search-input");
const resultsEl = $("results");
const recommendedEl = $("recommended");
const queueListEl = $("queue-list");
const statusEl = $("status");
const npTitle = $("np-title");
const npThumb = $("np-thumb");
const npMode = $("np-mode");
const welcomeTitle = $("welcome-title");
const welcomeText = $("welcome-text");
const resultsTitle = $("results-title");
const panelWelcome = $("panel-welcome");
const panelResults = $("panel-results");
const panelRecommended = $("panel-recommended");
const panelLive = $("panel-live");
const panelPlaylist = $("panel-playlist");
const panelVideo = $("panel-video");
const videoStage = $("video-stage");
const liveResultsEl = $("live-results");
const recommendedSubtitle = $("recommended-subtitle");
const playlistResultsEl = $("playlist-results");
const playlistSubtitle = $("playlist-subtitle");

let mode = "music";
let loggedIn = false;
let lastVideoId = null;
let currentPlaylistItems = [];
const feedCache = { music: null, video: null };
let recLoading = false;

const LABELS = {
  music: {
    mode: "AUDIO.SYS",
    placeholder: "_buscar música...",
    welcome: "modo áudio ativo — stream via mpv, sem janela de vídeo.",
    playing: "STREAM ATIVO",
    searchTitle: "Resultados // áudio",
  },
  video: {
    mode: "VIDEO.SYS",
    placeholder: "_buscar vídeo...",
    welcome: "modo vídeo — player integrado na janela do app.",
    playing: "STREAM ATIVO",
    searchTitle: "Resultados // vídeo",
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
  if (welcomeTitle) welcomeTitle.textContent = `// ${L.mode}`;
  if (welcomeText) welcomeText.textContent = L.welcome;
  if (resultsTitle) resultsTitle.textContent = L.searchTitle;
  focusSearch();
}

function showWelcome() {
  panelWelcome?.classList.remove("hidden");
  panelResults?.classList.add("hidden");
  panelRecommended?.classList.add("hidden");
  panelLive?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
  panelVideo?.classList.add("hidden");
}

function showResults() {
  panelWelcome?.classList.add("hidden");
  panelResults?.classList.remove("hidden");
  panelLive?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
}

function showHomeFeed() {
  panelWelcome?.classList.add("hidden");
  panelResults?.classList.add("hidden");
  panelPlaylist?.classList.add("hidden");
}

function showPlaylist() {
  panelWelcome?.classList.add("hidden");
  panelPlaylist?.classList.remove("hidden");
}

function showRecommended() {
  panelRecommended?.classList.remove("hidden");
}

function showLive() {
  panelLive?.classList.remove("hidden");
}

function renderCards(container, items, emptyMsg) {
  if (!container) return;
  container.replaceChildren();
  if (!items?.length) {
    const p = document.createElement("p");
    p.className = "muted";
    p.textContent = emptyMsg;
    container.appendChild(p);
    return;
  }
  for (const v of items) {
    const card = document.createElement("article");
    card.className = "card" + (v.is_live ? " card-live" : "");
    const thumb = v.thumbnail || `https://i.ytimg.com/vi/${v.id}/hqdefault.jpg`;

    const img = document.createElement("img");
    img.src = thumb;
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
    container.appendChild(card);
  }
}

async function tauriInvoke(cmd, args = {}) {
  if (!isTauri()) {
    throw new Error("Abra pelo app promptub (não pelo navegador)");
  }
  return invoke(cmd, args);
}

async function search() {
  if (!searchInput) return;
  const q = searchInput.value.trim();
  if (!q) return;
  setStatus("BUSCANDO...");
  showResults();
  try {
    const res = await tauriInvoke("search", { query: q });
    renderCards(resultsEl, res, "Nenhum resultado");
    setStatus(`${res.length} HIT${res.length === 1 ? "" : "S"}`);
  } catch (e) {
    setStatus(`Erro: ${e}`);
  }
}

async function syncVideoPanel() {
  if (!videoStage || isAudioOnly()) return false;
  const r = videoStage.getBoundingClientRect();
  if (r.width < 10 || r.height < 10) return false;
  await tauriInvoke("sync_video_panel", {
    x: r.left,
    y: r.top,
    width: r.width,
    height: r.height,
  });
  return true;
}

async function hideVideoPanel() {
  panelVideo?.classList.add("hidden");
  try {
    await tauriInvoke("hide_video_panel");
  } catch (_) {}
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
      panelVideo?.classList.remove("hidden");
      showHomeFeed();
      await new Promise((r) => requestAnimationFrame(r));
      await new Promise((r) => setTimeout(r, 50));
      let synced = false;
      for (let i = 0; i < 5 && !synced; i++) {
        synced = await syncVideoPanel().catch(() => false);
        if (!synced) await new Promise((r) => setTimeout(r, 80));
      }
      if (!synced) throw new Error("painel de video indisponivel");
    } else {
      await hideVideoPanel();
    }
    await tauriInvoke("play", { video, setQueue, audioOnly: isAudioOnly() });
    if (!isAudioOnly()) {
      setTimeout(() => syncVideoPanel().catch(() => {}), 200);
    }
    if (npTitle) npTitle.textContent = video.title;
    if (npThumb) {
      npThumb.src = video.thumbnail || `https://i.ytimg.com/vi/${video.id}/hqdefault.jpg`;
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

function renderHomeFeed(feed) {
  renderCards(recommendedEl, feed.recommended, "sem recomendados");
  if (recommendedSubtitle) {
    const n = feed.recommended?.length || 0;
    recommendedSubtitle.textContent = feed.seed_label
      ? `baseado em: ${feed.seed_label} · ${n} itens`
      : `${n} itens`;
  }
  if (feed.recommended?.length) showRecommended();
  else panelRecommended?.classList.add("hidden");

  if (mode === "video" && feed.live?.length) {
    renderCards(liveResultsEl, feed.live, "sem lives no momento");
    showLive();
  } else {
    panelLive?.classList.add("hidden");
  }

  if (feed.recommended?.length || feed.live?.length) showHomeFeed();
  else showWelcome();
}

async function loadHomeFeed(force = false) {
  if (recLoading) return;
  if (!force && feedCache[mode]) {
    renderHomeFeed(feedCache[mode]);
    setStatus(`REC CACHE — ${feedCache[mode].recommended?.length || 0}`);
    return;
  }

  const btn = $("btn-refresh-rec");
  recLoading = true;
  if (btn) btn.disabled = true;
  setStatus("FETCH REC...");
  try {
    const feed = await tauriInvoke("home_recommendations", { mode });
    feedCache[mode] = feed;
    renderHomeFeed(feed);
    setStatus(`REC OK — ${feed.recommended?.length || 0}`);
  } catch (e) {
    showWelcome();
    setStatus(String(e));
  } finally {
    recLoading = false;
    if (btn) btn.disabled = false;
  }
}

async function buildRecommendedPlaylist() {
  const btn = $("btn-rec-playlist");
  if (btn) btn.disabled = true;
  setStatus("BUILDING REC.PL...");
  try {
    const res = await tauriInvoke("recommended_playlist", {
      seedVideoId: lastVideoId || null,
      seedQuery: searchInput?.value.trim() || null,
    });
    currentPlaylistItems = res.items || [];
    if (playlistSubtitle) {
      playlistSubtitle.textContent = `Baseado em: ${res.seed_label} · ${res.count} faixas`;
    }
    renderCards(playlistResultsEl, currentPlaylistItems, "Nenhuma faixa");
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
    const v = await tauriInvoke("play_queue_item", { index });
    if (v) {
      lastVideoId = v.id;
      if (npTitle) npTitle.textContent = v.title;
      if (npThumb) {
        npThumb.src = v.thumbnail || `https://i.ytimg.com/vi/${v.id}/hqdefault.jpg`;
        npThumb.classList.remove("hidden");
      }
      if (!isAudioOnly()) {
        panelVideo?.classList.remove("hidden");
        await new Promise((r) => requestAnimationFrame(r));
        await syncVideoPanel();
      }
      setStatus(LABELS[mode].playing);
      refreshQueue();
    }
  } catch (e) {
    setStatus(String(e));
  }
}

async function refreshQueue() {
  try {
    const q = await tauriInvoke("get_queue");
    if (!queueListEl) return;
    queueListEl.replaceChildren();
    q.items.forEach((item, i) => {
      const li = document.createElement("li");
      li.className = "queue-item";
      if (i === q.current) li.classList.add("playing");

      const title = document.createElement("span");
      title.className = "queue-item-title";
      title.textContent = item.title;
      title.title = item.title;
      title.onclick = () => playFromQueue(i);

      const btnRemove = document.createElement("button");
      btnRemove.className = "btn-queue-remove";
      btnRemove.type = "button";
      btnRemove.title = "Remover da fila";
      btnRemove.textContent = "×";
      btnRemove.onclick = (e) => {
        e.stopPropagation();
        removeFromQueue(i);
      };

      li.append(title, btnRemove);
      queueListEl.appendChild(li);
    });
  } catch (_) {}
}

async function init() {
  applyMode("music");
  showWelcome();
  focusSearch();

  if (!isTauri()) {
    setStatus("Modo preview — use o app promptub");
    return;
  }
  try {
    await tauriInvoke("check_deps");
    loggedIn = await tauriInvoke("is_logged_in");
    const badge = $("premium-badge");
    const btnPremium = $("btn-premium");
    if (badge) badge.textContent = loggedIn ? "PREMIUM OK" : "MODO GRATUITO";
    if (btnPremium) btnPremium.textContent = loggedIn ? "[ LOGOUT ]" : "[ AUTH ] PREMIUM";
    setStatus("PRONTO — use [ ATUALIZAR ] para recomendados");
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
      renderHomeFeed(feedCache[mode]);
    } else {
      showWelcome();
    }
  });
});

let resizeTimer;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    if (!isAudioOnly() && panelVideo && !panelVideo.classList.contains("hidden")) {
      syncVideoPanel();
    }
  }, 120);
});

$("btn-search")?.addEventListener("click", search);
$("btn-refresh-rec")?.addEventListener("click", () => loadHomeFeed(true));
$("btn-rec-playlist")?.addEventListener("click", buildRecommendedPlaylist);
$("btn-playlist-to-queue")?.addEventListener("click", sendPlaylistToQueue);

$("volume-slider")?.addEventListener("input", async (e) => {
  try {
    await tauriInvoke("set_volume", { level: Number(e.target.value) });
  } catch (_) {}
});

searchInput?.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    search();
  }
});

$("btn-stop")?.addEventListener("click", async () => {
  try {
    await tauriInvoke("stop");
    await hideVideoPanel();
    setStatus("STREAM STOPPED");
    focusSearch();
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-next")?.addEventListener("click", async () => {
  try {
    const v = await tauriInvoke("next");
    if (v) await play(v, false);
  } catch (e) {
    setStatus(String(e));
  }
});

$("btn-prev")?.addEventListener("click", async () => {
  try {
    const v = await tauriInvoke("prev");
    if (v) await play(v, false);
  } catch (e) {
    setStatus(String(e));
  }
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
