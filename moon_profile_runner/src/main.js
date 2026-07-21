// "Clients" card (index.html): lists every Deck that has ever connected
// to this Runner (persisted on the Rust side, see clients.rs) and, only
// while THIS window is open and visible, pings each one every 3s to show
// its current latency. Deliberately driven from here (the window's own
// script) and not from the always-on HTTP server thread (server.rs) -
// see docs/prd.md - so an unreachable Deck can only ever slow down this
// window, never anything the Deck itself is waiting on. Paused via the
// Page Visibility API while the window is minimized/unfocused, resumed
// when it becomes visible again.

const { invoke } = window.__TAURI__.core;

const PING_INTERVAL_MS = 3000;

const listEl = document.querySelector("#clients-list");
const emptyEl = document.querySelector("#clients-empty");

function formatLastSeen(isoString) {
  const parsed = new Date(isoString);
  return Number.isNaN(parsed.getTime()) ? isoString : parsed.toLocaleString();
}

// latencies: Map<ip, number | null> - null means "pinged, unreachable";
// an ip missing from the map means "not pinged yet this refresh".
function renderClients(clients, latencies) {
  emptyEl.style.display = clients.length === 0 ? "block" : "none";
  listEl.innerHTML = "";

  for (const client of clients) {
    const row = document.createElement("li");
    row.className = "client-row";

    const label = document.createElement("span");
    label.className = "client-label";
    label.textContent = `${client.ip} (last seen ${formatLastSeen(client.last_seen)})`;

    const latency = document.createElement("span");
    const ms = latencies.get(client.ip);
    if (ms === undefined) {
      latency.className = "client-latency client-latency-pending";
      latency.textContent = "...";
    } else if (ms === null) {
      latency.className = "client-latency client-latency-down";
      latency.textContent = "Could not connect";
    } else {
      latency.className = "client-latency client-latency-up";
      latency.textContent = `${ms.toFixed(1)} ms`;
    }

    row.append(label, latency);
    listEl.append(row);
  }
}

async function refreshClients() {
  const clients = await invoke("list_known_clients");
  const latencies = new Map();
  renderClients(clients, latencies);

  // Pings run concurrently (not one-by-one) and each re-render happens
  // as its own result comes back, so a single slow/unreachable Deck
  // doesn't delay the others from showing their latency.
  await Promise.all(
    clients.map(async (client) => {
      const ms = await invoke("ping_client", { ip: client.ip });
      latencies.set(client.ip, ms ?? null);
      renderClients(clients, latencies);
    }),
  );
}

let pollHandle = null;

function startPolling() {
  if (pollHandle !== null) {
    return;
  }
  refreshClients();
  pollHandle = setInterval(refreshClients, PING_INTERVAL_MS);
}

function stopPolling() {
  if (pollHandle === null) {
    return;
  }
  clearInterval(pollHandle);
  pollHandle = null;
}

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") {
    startPolling();
  } else {
    stopPolling();
  }
});

if (document.visibilityState === "visible") {
  startPolling();
}
