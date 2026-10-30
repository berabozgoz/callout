<script lang="ts">
  import { onMount } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    send, getSettings, saveSettings, listDevices, appVersion, onEngine, onPtt, prettyCode,
    engineSnapshot, pttCapture, prettyShortcut,
    type Settings, type Device, type Member, type RoomInfo, type ConnectionState,
  } from "./engine";

  // ---- durum ----
  let settings = $state<Settings>({ nickname: "", last_invite: "", input_device: null, output_device: null, push_to_talk: false, ptt_shortcut: "F9" });
  let devices = $state<Device[]>([]);
  let version = $state("");
  let conn = $state<ConnectionState>("disconnected");
  let myId = $state<number | null>(null);
  let room = $state<RoomInfo | null>(null);
  let members = $state<Member[]>([]);
  let speaking = $state<Set<number>>(new Set());
  let volumes = $state<Record<number, number>>({});
  let level = $state(0);
  let transmitting = $state(false);
  let muted = $state(false);
  let deafened = $state(false);
  let stats = $state({ rtt_ms: 0, jitter_ms: 0, lost: 0 });
  let error = $state("");
  let joinCode = $state("");
  let roomName = $state("");
  let showSettings = $state(false);
  let copied = $state(false);
  let capturing = $state(false);

  const inputs = $derived(devices.filter((d) => d.direction === "input"));
  const outputs = $derived(devices.filter((d) => d.direction === "output"));
  const me = $derived(members.find((m) => m.user_id === myId));

  onMount(async () => {
    version = await appVersion();
    settings = await getSettings();
    try { devices = await listDevices(); } catch (e) { error = String(e); }

    await onEngine((e) => {
      switch (e.type) {
        case "connection_state":
          conn = e.state;
          if (e.state === "disconnected") { room = null; members = []; myId = null; speaking = new Set(); }
          break;
        case "connected": myId = e.user_id; error = ""; break;
        case "room_updated":
          room = e.room; members = e.members;
          if (!e.room) speaking = new Set();
          break;
        case "speaking_changed": {
          const s = new Set(speaking);
          e.speaking ? s.add(e.user_id) : s.delete(e.user_id);
          speaking = s;
          break;
        }
        case "input_level": level = e.level; transmitting = e.transmitting; break;
        case "stats": stats = { rtt_ms: e.rtt_ms, jitter_ms: e.jitter_ms, lost: e.lost }; break;
        case "audio_restored":
          error = e.fallback_to_default
            ? "Seçili ses cihazı bulunamadı; varsayılan cihaza geçildi."
            : error.startsWith("Ses cihazı") ? "" : error;
          break;
        case "error": error = e.message; break;
      }
    });
    await onPtt((pressed) => { transmitting = pressed && !muted; });

    // Webview yenilenmiş olabilir (F5 vb.): motor hâlâ bağlıysa durumu ondan al.
    // Dinleyiciler kurulduktan sonra okunur ki aradaki olaylar kaçmasın.
    try {
      const s = await engineSnapshot();
      conn = s.connection; myId = s.user_id; room = s.room; members = s.members;
      muted = s.muted; deafened = s.deafened;
    } catch (e) { error = String(e); }
  });

  // Yayın sürümünde yenileme tuşları ve sağ tık menüsü kapalı: yenileme arayüz durumunu sıfırlardı.
  const MODIFIER_CODES = new Set(["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight", "AltGraph"]);
  function onWindowKey(e: KeyboardEvent) {
    if (capturing) { captureKey(e); return; }
    if (!import.meta.env.PROD) return;
    const reload = e.code === "F5" || (e.ctrlKey && e.code === "KeyR");
    if (reload) e.preventDefault();
  }
  function onContextMenu(e: MouseEvent) { if (import.meta.env.PROD) e.preventDefault(); }

  // ---- bas-konuş tuşu yakalama ----
  async function startCapture() {
    capturing = true;
    try { await pttCapture(true); } catch (e) { error = String(e); }
  }
  async function stopCapture() {
    capturing = false;
    try { await pttCapture(false); } catch (e) { error = String(e); }
  }
  function captureKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.repeat || MODIFIER_CODES.has(e.code)) return; // yalnız değiştirici: asıl tuşu bekle
    if (e.code === "Escape") { stopCapture(); return; }
    if (e.code === "Backspace" || e.code === "Delete") { settings.ptt_shortcut = ""; stopCapture(); return; }
    const parts: string[] = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    if (e.metaKey) parts.push("Super");
    parts.push(e.code); // KeyT, Digit9, F9, ArrowUp… kısayol kütüphanesinin kendi adları
    settings.ptt_shortcut = parts.join("+");
    stopCapture();
  }

  // ---- eylemler ----
  async function connect() {
    error = "";
    settings = { ...settings, nickname: settings.nickname.trim(), last_invite: settings.last_invite.trim() };
    if (!settings.nickname) { error = "Takma ad gerekli"; return; }
    if (!settings.last_invite.startsWith("callout://")) { error = "Davet linki callout:// ile başlamalı"; return; }
    await saveSettings(settings);
    await send({ type: "connect", invite: settings.last_invite, nickname: settings.nickname });
  }
  const disconnect = () => send({ type: "disconnect" });
  const createRoom = () => send({ type: "create_room", name: roomName.trim() });
  const joinRoom = () => { if (joinCode.trim()) send({ type: "join_room", code: joinCode.trim() }); };
  const leaveRoom = () => send({ type: "leave_room" });
  function toggleMute() { muted = !muted; send({ type: "set_muted", muted }); }
  function toggleDeafen() { deafened = !deafened; send({ type: "set_deafened", deafened }); }
  function setVolume(user_id: number, gain: number) {
    volumes = { ...volumes, [user_id]: gain };
    send({ type: "set_user_volume", user_id, gain });
  }
  async function copyCode() {
    if (!room) return;
    await writeText(prettyCode(room.code));
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }
  async function applySettings() {
    if (capturing) await stopCapture();
    try {
      await saveSettings(settings);
      showSettings = false;
    } catch (e) { error = String(e); }
  }
  async function cancelSettings() {
    if (capturing) await stopCapture();
    showSettings = false;
  }
</script>

<svelte:window onkeydowncapture={onWindowKey} oncontextmenu={onContextMenu} />

<main>
  <header>
    <h1>CallOut</h1>
    <div class="right">
      {#if conn === "connected"}
        <span class="pill ok">bağlı · {stats.rtt_ms} ms</span>
      {:else if conn === "connecting"}
        <span class="pill">bağlanıyor…</span>
      {/if}
      <button class="icon" title="Ayarlar" onclick={() => (showSettings = !showSettings)}>⚙</button>
    </div>
  </header>

  {#if error}
    <button class="error" type="button" onclick={() => (error = "")}>{error}</button>
  {/if}

  {#if showSettings}
    <section class="card">
      <h2>Ayarlar</h2>
      <label>Mikrofon
        <select bind:value={settings.input_device}>
          <option value={null}>Varsayılan</option>
          {#each inputs as d (d.name)}<option value={d.name}>{d.name}</option>{/each}
        </select>
      </label>
      <label>Hoparlör
        <select bind:value={settings.output_device}>
          <option value={null}>Varsayılan</option>
          {#each outputs as d (d.name)}<option value={d.name}>{d.name}</option>{/each}
        </select>
      </label>
      <label class="row">
        <input type="checkbox" bind:checked={settings.push_to_talk} />
        Bas-konuş (kapalıysa ses algılama)
      </label>
      {#if settings.push_to_talk}
        <label>Bas-konuş tuşu
          <button type="button" class="capture" class:listening={capturing} onclick={(e) => { if (e.detail === 0) return; /* Space/Enter ile yakalanan tuşun tıklamaya dönüşmesini yok say */ capturing ? stopCapture() : startCapture(); }}>
            {capturing ? "Bir tuşa ya da kombinasyona bas… (Esc iptal)" : settings.ptt_shortcut ? prettyShortcut(settings.ptt_shortcut) : "Atanmamış — tıkla ve bir tuşa bas"}
          </button>
        </label>
        <p class="hint">Kısayol yalnızca bir odadayken etkindir. Düz bir harf (ör. T) seçersen odadayken diğer uygulamalarda o tuşla yazamazsın; F9 ya da Ctrl+Alt gibi birleşimler daha güvenlidir.</p>
      {/if}
      <div class="actions">
        <button class="primary" onclick={applySettings}>Kaydet</button>
        <button onclick={cancelSettings}>Vazgeç</button>
      </div>
    </section>
  {:else if conn !== "connected"}
    <!-- ---------- Bağlan ---------- -->
    <section class="card">
      <h2>Sunucuya bağlan</h2>
      <label>Takma ad
        <input type="text" bind:value={settings.nickname} placeholder="bera" maxlength="24" />
      </label>
      <label>Davet linki
        <input type="text" bind:value={settings.last_invite} placeholder="callout://IP:4433#parmak-izi" />
      </label>
      <div class="actions">
        <button class="primary" disabled={conn === "connecting"} onclick={connect}>
          {conn === "connecting" ? "Bağlanıyor…" : "Bağlan"}
        </button>
      </div>
      <p class="hint">Davet linkini sunucuyu çalıştıran arkadaşından al. Sunucu ilk açılışta linki ekrana basar.</p>
    </section>
  {:else if !room}
    <!-- ---------- Lobi ---------- -->
    <section class="card">
      <h2>Oda aç</h2>
      <label>Oda adı (isteğe bağlı)
        <input type="text" bind:value={roomName} placeholder="takım" maxlength="32" />
      </label>
      <div class="actions"><button class="primary" onclick={createRoom}>Oda aç</button></div>
    </section>
    <section class="card">
      <h2>Odaya gir</h2>
      <label>Oda kodu
        <input type="text" bind:value={joinCode} placeholder="ABCD-1234" class="code" onkeydown={(e) => e.key === "Enter" && joinRoom()} />
      </label>
      <div class="actions">
        <button class="primary" onclick={joinRoom}>Gir</button>
        <button onclick={disconnect}>Bağlantıyı kes</button>
      </div>
    </section>
  {:else}
    <!-- ---------- Oda ---------- -->
    <section class="card room">
      <div class="roomhead">
        <div>
          <h2>{room.name}</h2>
          <button class="codebtn" title="Kodu kopyala" onclick={copyCode}>
            {prettyCode(room.code)} {copied ? "✓" : "⧉"}
          </button>
        </div>
        <button onclick={leaveRoom}>Ayrıl</button>
      </div>

      <ul class="members">
        {#each members as m (m.user_id)}
          <li class:speaking={speaking.has(m.user_id)} class:me={m.user_id === myId}>
            <span class="ring"></span>
            <span class="name">{m.nickname}{m.user_id === myId ? " (sen)" : ""}</span>
            <span class="flags">{m.muted ? "🔇" : ""}{m.deafened ? "🙉" : ""}</span>
            {#if m.user_id !== myId}
              <input class="vol" type="range" min="0" max="2" step="0.05"
                value={volumes[m.user_id] ?? 1}
                oninput={(e) => setVolume(m.user_id, Number((e.target as HTMLInputElement).value))}
                title="Ses: {Math.round((volumes[m.user_id] ?? 1) * 100)}%" />
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <section class="bar">
      <div class="meter" class:tx={transmitting}>
        <div class="fill" style="width: {Math.round(level * 100)}%"></div>
      </div>
      <button class:active={muted} onclick={toggleMute} title="Mikrofonu sustur">{muted ? "🔇" : "🎙"}</button>
      <button class:active={deafened} onclick={toggleDeafen} title="Sesi kapat">{deafened ? "🙉" : "🎧"}</button>
      <span class="stats">{stats.rtt_ms} ms · tampon {stats.jitter_ms} ms{stats.lost ? ` · kayıp ${stats.lost}` : ""}</span>
    </section>
    {#if settings.push_to_talk}
      <p class="hint">Bas-konuş: <b>{settings.ptt_shortcut}</b> basılıyken gönderilir.</p>
    {/if}
  {/if}

  <footer>v{version}{me?.muted ? " · sessiz" : ""}</footer>
</main>
