<script lang="ts">
  import { onMount } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    send, getSettings, saveSettings, listDevices, appVersion, onEngine, onPtt, prettyCode,
    engineSnapshot, pttCapture, prettyShortcut, setTrayLabels, isInstalled, openReleasesPage,
    type Settings, type Device, type Member, type RoomInfo, type ConnectionState, type MicTestState,
    type ErrorKind,
  } from "./engine";
  import { t, i18n, setLanguage, LANGUAGES } from "./i18n/index.svelte";
  import Icon from "./Icon.svelte";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import type { MessageKey } from "./i18n/en";

  /** Errors are stored untranslated so they follow a language change. */
  type UiError = { key: MessageKey; vars?: Record<string, string | number>; kind?: ErrorKind };
  const fail = (e: unknown): UiError => ({ key: "error.generic", vars: { detail: String(e) } });

  // ---- state ----
  let settings = $state<Settings>({ nickname: "", last_invite: "", input_device: null, output_device: null, push_to_talk: false, ptt_shortcut: "F9", noise_suppression: true, vad_sensitivity: 0, language: null });
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
  let threshold = $state(0);
  // Microphone sample rate from the last audio pipeline start; null = no microphone (listening only),
  // undefined = no pipeline yet.
  let inputRate = $state<number | null | undefined>(undefined);
  let muted = $state(false);
  let deafened = $state(false);
  let stats = $state({ rtt_ms: 0, jitter_ms: 0, lost: 0 });
  let error = $state<UiError | null>(null);
  let joinCode = $state("");
  let roomName = $state("");
  let showSettings = $state(false);
  let copied = $state(false);
  let capturing = $state(false);
  // Saved state when settings open: "Cancel" reverts the devices picked (tried out) meanwhile.
  let savedSettings: Settings | null = null;
  let micTest = $state<MicTestState>("idle");
  let micResult = $state<number | null>(null);
  let recStart = $state(0);
  let now = $state(0);

  const inputs = $derived(devices.filter((d) => d.direction === "input"));
  const outputs = $derived(devices.filter((d) => d.direction === "output"));
  const me = $derived(members.find((m) => m.user_id === myId));
  const remaining = $derived(Math.max(1, 5 - Math.floor((now - recStart) / 1000)));

  // ---- updates (GitHub Releases, signed) ----
  let update = $state.raw<Update | null>(null);
  let updating = $state<"idle" | "downloading" | "failed">("idle");
  let updatePct = $state(0);
  // Portable exe: the updater would install a second copy instead of replacing this one.
  let installed = $state(true);
  async function checkForUpdate() {
    try {
      const u = await check();
      if (u) update = u;
    } catch (e) {
      // Offline, no release yet, dev build: not worth bothering the user.
      console.warn("update check failed", e);
    }
  }
  async function installUpdate() {
    if (!update) return;
    updating = "downloading";
    updatePct = 0;
    let total = 0;
    let got = 0;
    try {
      await update.download((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          got += ev.data.chunkLength;
          updatePct = total ? Math.round((got * 100) / total) : 0;
        }
      });
      // The installer closes the app: leave the server cleanly first instead of timing out.
      if (conn !== "disconnected") {
        await send({ type: "disconnect" });
        await new Promise((r) => setTimeout(r, 700));
      }
      await update.install();
    } catch (e) {
      console.error("update failed", e);
      updating = "failed";
    }
  }

  // Tray menu follows the UI language.
  $effect(() => {
    void i18n.lang;
    setTrayLabels(t("tray.show"), t("tray.quit")).catch(() => {});
  });

  // Countdown while recording.
  $effect(() => {
    if (micTest !== "recording") return;
    const t = setInterval(() => (now = Date.now()), 200);
    return () => clearInterval(t);
  });

  onMount(async () => {
    version = await appVersion();
    settings = await getSettings();
    setLanguage(settings.language);
    if (import.meta.env.PROD) {
      try { installed = await isInstalled(); } catch { installed = true; }
      checkForUpdate();
      setInterval(checkForUpdate, 6 * 60 * 60 * 1000);
    }
    try { devices = await listDevices(); } catch (e) { error = fail(e); }

    await onEngine((e) => {
      switch (e.type) {
        case "connection_state":
          conn = e.state;
          if (e.state === "disconnected") { room = null; members = []; myId = null; speaking = new Set(); }
          break;
        case "connected": myId = e.user_id; error = null; break;
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
        case "input_level": level = e.level; transmitting = e.transmitting; threshold = e.threshold; break;
        case "stats": stats = { rtt_ms: e.rtt_ms, jitter_ms: e.jitter_ms, lost: e.lost }; break;
        case "audio_restored":
          error = e.fallback_to_default
            ? { key: "audio.fallback" }
            : error?.kind === "audio_device_lost" ? null : error;
          break;
        case "audio_started": inputRate = e.input_rate; break;
        case "mic_test":
          micTest = e.state;
          if (e.state === "recording") { recStart = now = Date.now(); micResult = null; }
          if (e.state === "idle" && e.transmitted_pct !== null) micResult = e.transmitted_pct;
          break;
        case "error":
          error = { key: `error.${e.kind}` as MessageKey, vars: { detail: e.message }, kind: e.kind };
          break;
      }
    });
    await onPtt((pressed) => { transmitting = pressed && !muted; });

    // The webview may have been reloaded (F5 etc.): if the engine is still connected, take the state from it.
    // Read after the listeners are set up so no events in between are missed.
    try {
      const s = await engineSnapshot();
      conn = s.connection; myId = s.user_id; room = s.room; members = s.members;
      muted = s.muted; deafened = s.deafened;
      if (s.input_rate !== undefined) inputRate = s.input_rate;
    } catch (e) { error = fail(e); }
  });

  // In release builds the reload keys and the context menu are disabled: a reload would reset the UI state.
  const MODIFIER_CODES = new Set(["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight", "AltGraph"]);
  function onWindowKey(e: KeyboardEvent) {
    if (capturing) { captureKey(e); return; }
    if (!import.meta.env.PROD) return;
    const reload = e.code === "F5" || (e.ctrlKey && e.code === "KeyR");
    if (reload) e.preventDefault();
  }
  function onContextMenu(e: MouseEvent) { if (import.meta.env.PROD) e.preventDefault(); }

  // ---- push-to-talk key capture ----
  async function startCapture() {
    capturing = true;
    try { await pttCapture(true); } catch (e) { error = fail(e); }
  }
  async function stopCapture() {
    capturing = false;
    try { await pttCapture(false); } catch (e) { error = fail(e); }
  }
  function captureKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.repeat || MODIFIER_CODES.has(e.code)) return; // modifier only: wait for the actual key
    if (e.code === "Escape") { stopCapture(); return; }
    if (e.code === "Backspace" || e.code === "Delete") { settings.ptt_shortcut = ""; stopCapture(); return; }
    const parts: string[] = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.altKey) parts.push("Alt");
    if (e.shiftKey) parts.push("Shift");
    if (e.metaKey) parts.push("Super");
    parts.push(e.code); // KeyT, Digit9, F9, ArrowUp… the shortcut library's own names
    settings.ptt_shortcut = parts.join("+");
    stopCapture();
  }

  // ---- actions ----
  async function connect() {
    error = null;
    settings = { ...settings, nickname: settings.nickname.trim(), last_invite: settings.last_invite.trim() };
    if (!settings.nickname) { error = { key: "connect.nicknameRequired" }; return; }
    if (!settings.last_invite.startsWith("callout://")) { error = { key: "connect.inviteInvalid" }; return; }
    await saveSettings(settings);
    await send({ type: "connect", invite: settings.last_invite, nickname: settings.nickname });
  }
  const disconnect = () => send({ type: "disconnect" });
  // An empty name gets a default in the creator's language (the server's own fallback is English).
  const defaultRoomName = $derived(t("lobby.defaultRoomName", { name: settings.nickname.trim() }));
  const createRoom = () => send({ type: "create_room", name: roomName.trim() || defaultRoomName });
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
  // ---- settings and sound test ----
  function openSettings() {
    savedSettings = { ...settings };
    micResult = null;
    showSettings = true;
    // Open the audio pipeline while disconnected so the level meter and the test work.
    send({ type: "set_audio_preview", enabled: true });
  }
  function closeSettings() {
    showSettings = false;
    if (micTest !== "idle") send({ type: "stop_mic_test" });
    send({ type: "set_audio_preview", enabled: false });
  }
  async function applySettings() {
    if (capturing) await stopCapture();
    try {
      await saveSettings(settings);
      closeSettings();
    } catch (e) { error = fail(e); }
  }
  async function cancelSettings() {
    if (capturing) await stopCapture();
    if (savedSettings) {
      // Revert devices that were tried but not saved.
      if (savedSettings.input_device !== settings.input_device) send({ type: "set_input_device", name: savedSettings.input_device });
      if (savedSettings.output_device !== settings.output_device) send({ type: "set_output_device", name: savedSettings.output_device });
      if (savedSettings.noise_suppression !== settings.noise_suppression) send({ type: "set_noise_suppression", enabled: savedSettings.noise_suppression });
      if (savedSettings.vad_sensitivity !== settings.vad_sensitivity) send({ type: "set_vad_sensitivity", db: savedSettings.vad_sensitivity });
      if (savedSettings.language !== settings.language) setLanguage(savedSettings.language);
      settings = savedSettings;
    }
    closeSettings();
  }
  // The picked device is applied without saving so the test uses it. The selection is read from DOM order
  // (0 = Default); it does not depend on when bind:value updates.
  function pickInput(e: Event) {
    const i = (e.currentTarget as HTMLSelectElement).selectedIndex;
    send({ type: "set_input_device", name: i > 0 ? inputs[i - 1].name : null });
  }
  function pickOutput(e: Event) {
    const i = (e.currentTarget as HTMLSelectElement).selectedIndex;
    send({ type: "set_output_device", name: i > 0 ? outputs[i - 1].name : null });
  }
  function toggleMicTest() {
    if (micTest === "idle") { micResult = null; send({ type: "start_mic_test" }); }
    else send({ type: "stop_mic_test" });
  }
  const playTestSound = () => send({ type: "play_test_sound" });
</script>

<svelte:window onkeydowncapture={onWindowKey} oncontextmenu={onContextMenu} />

{#snippet inputHint()}
  {#if inputRate === null}
    <p class="hint warn">{t("audio.listenOnly")}</p>
  {:else if inputRate !== undefined && inputRate <= 16000}
    <p class="hint warn">{t("audio.bluetoothMic", { khz: inputRate / 1000 })}</p>
  {/if}
{/snippet}

{#snippet meter()}
  <div class="meter" class:tx={transmitting}>
    <div class="fill" style="width: {Math.round(level * 100)}%"></div>
    {#if !settings.push_to_talk}
      <div class="thr" style="inset-inline-start: {Math.round(threshold * 100)}%" title={t("test.threshold")}></div>
    {/if}
  </div>
{/snippet}

<main>
  <header>
    <h1>CallOut</h1>
    <div class="right">
      {#if conn === "connected"}
        <span class="pill ok">{t("header.connected", { ms: stats.rtt_ms })}</span>
      {:else if conn === "connecting"}
        <span class="pill">{t("header.connecting")}</span>
      {:else if conn === "reconnecting"}
        <span class="pill warn">{t("header.reconnecting")}</span>
      {/if}
      <button class="icon" title={t("header.settings")} onclick={() => (showSettings ? cancelSettings() : openSettings())}>⚙</button>
    </div>
  </header>

  {#if update}
    <div class="update">
      {#if !installed}
        <span>{t("update.availablePortable", { version: update.version })}</span>
        <button class="primary" onclick={() => openReleasesPage().catch(() => {})}>{t("update.download")}</button>
      {:else}
        <span>
          {updating === "downloading" ? t("update.downloading", { pct: updatePct })
            : updating === "failed" ? t("update.failed")
            : t("update.available", { version: update.version })}
        </span>
        {#if updating !== "downloading"}
          <button class="primary" onclick={installUpdate}>{t("update.install")}</button>
        {/if}
      {/if}
    </div>
  {/if}

  {#if error}
    <button class="error" type="button" onclick={() => (error = null)}>{t(error.key, error.vars)}</button>
  {/if}

  {#if showSettings}
    <section class="card">
      <h2>{t("settings.title")}</h2>
      <label>{t("settings.language")}
        <select bind:value={settings.language} onchange={() => setLanguage(settings.language)}>
          <option value={null}>{t("settings.languageAuto")}</option>
          {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.name}</option>{/each}
        </select>
      </label>
      <label>{t("settings.microphone")}
        <select bind:value={settings.input_device} onchange={pickInput}>
          <option value={null}>{t("settings.default")}</option>
          {#each inputs as d (d.name)}<option value={d.name}>{d.name}</option>{/each}
        </select>
      </label>
      <label>{t("settings.speaker")}
        <select bind:value={settings.output_device} onchange={pickOutput}>
          <option value={null}>{t("settings.default")}</option>
          {#each outputs as d (d.name)}<option value={d.name}>{d.name}</option>{/each}
        </select>
      </label>
      <label class="row">
        <input type="checkbox" bind:checked={settings.noise_suppression}
          onchange={(e) => send({ type: "set_noise_suppression", enabled: (e.currentTarget as HTMLInputElement).checked })} />
        {t("settings.noiseSuppression")}
      </label>
      <label class="row">
        <input type="checkbox" bind:checked={settings.push_to_talk} />
        {t("settings.pushToTalk")}
      </label>
      {#if !settings.push_to_talk}
        <label>{t("settings.sensitivity")}
          <div class="sens">
            <span>{t("settings.sensitivityLow")}</span>
            <input type="range" min="-12" max="12" step="1" bind:value={settings.vad_sensitivity}
              oninput={() => send({ type: "set_vad_sensitivity", db: Number(settings.vad_sensitivity) })} />
            <span>{t("settings.sensitivityHigh")}</span>
          </div>
        </label>
      {/if}
      {#if settings.push_to_talk}
        <label>{t("settings.pttKey")}
          <button type="button" class="capture" class:listening={capturing} onclick={(e) => { if (e.detail === 0) return; /* ignore the click synthesized when Space/Enter is the captured key */ capturing ? stopCapture() : startCapture(); }}>
            {capturing ? t("settings.pttCapturing") : settings.ptt_shortcut ? prettyShortcut(settings.ptt_shortcut) : t("settings.pttUnassigned")}
          </button>
        </label>
        <p class="hint">{t("settings.pttHint")}</p>
      {/if}

      <div class="soundtest">
        <h3>{t("test.title")}</h3>
        {@render meter()}
        {@render inputHint()}
        <div class="actions">
          <button class:recording={micTest === "recording"} onclick={toggleMicTest}>
            {#if micTest === "calibrating"}{t("test.calibrating")}
            {:else if micTest === "recording"}{t("test.recording", { s: remaining })}
            {:else if micTest === "playing"}{t("test.playing")}
            {:else}{t("test.start")}{/if}
          </button>
          <button onclick={playTestSound}>{t("test.tone")}</button>
        </div>
        {#if micResult === 0}
          <p class="hint warn">{t("test.resultNone")}</p>
        {:else if micResult !== null}
          <p class="hint">{t("test.result", { pct: micResult })}</p>
        {:else if settings.push_to_talk}
          <p class="hint">{t("test.hintPtt")}</p>
        {:else}
          <p class="hint">{t("test.hintVad")}</p>
        {/if}
      </div>

      <div class="actions">
        <button class="primary" onclick={applySettings}>{t("settings.save")}</button>
        <button onclick={cancelSettings}>{t("settings.cancel")}</button>
      </div>
    </section>
  {:else if conn === "reconnecting"}
    <!-- ---------- Reconnecting ---------- -->
    <section class="card">
      <h2>{t("reconnect.title")}</h2>
      <p class="hint">{t("reconnect.hint")}</p>
      <div class="actions"><button onclick={disconnect}>{t("reconnect.cancel")}</button></div>
    </section>
  {:else if conn !== "connected"}
    <!-- ---------- Connect ---------- -->
    <section class="card">
      <h2>{t("connect.title")}</h2>
      <label>{t("connect.nickname")}
        <input type="text" bind:value={settings.nickname} placeholder={t("connect.nicknamePlaceholder")} maxlength="24" />
      </label>
      <label>{t("connect.invite")}
        <input type="text" bind:value={settings.last_invite} placeholder={t("connect.invitePlaceholder")} />
      </label>
      <div class="actions">
        <button class="primary" disabled={conn === "connecting"} onclick={connect}>
          {conn === "connecting" ? t("connect.connecting") : t("connect.button")}
        </button>
      </div>
      <p class="hint">{t("connect.hint")}</p>
    </section>
  {:else if !room}
    <!-- ---------- Lobby ---------- -->
    <section class="card">
      <h2>{t("lobby.create")}</h2>
      <label>{t("lobby.roomName")}
        <input type="text" bind:value={roomName} placeholder={defaultRoomName} maxlength="32" />
      </label>
      <div class="actions"><button class="primary" onclick={createRoom}>{t("lobby.createButton")}</button></div>
    </section>
    <section class="card">
      <h2>{t("lobby.join")}</h2>
      <label>{t("lobby.code")}
        <input type="text" bind:value={joinCode} placeholder="ABCD-1234" class="code" onkeydown={(e) => e.key === "Enter" && joinRoom()} />
      </label>
      <div class="actions">
        <button class="primary" onclick={joinRoom}>{t("lobby.joinButton")}</button>
        <button onclick={disconnect}>{t("lobby.disconnect")}</button>
      </div>
    </section>
  {:else}
    <!-- ---------- Room ---------- -->
    <section class="card room">
      <div class="roomhead">
        <div>
          <h2>{room.name}</h2>
          <button class="codebtn" title={t("room.copyCode")} onclick={copyCode}>
            {prettyCode(room.code)} {copied ? "✓" : "⧉"}
          </button>
        </div>
        <button onclick={leaveRoom}>{t("room.leave")}</button>
      </div>

      <ul class="members">
        {#each members as m (m.user_id)}
          <li class:speaking={speaking.has(m.user_id)} class:me={m.user_id === myId}>
            <span class="ring"></span>
            <span class="name">{m.nickname}{m.user_id === myId ? ` ${t("room.you")}` : ""}</span>
            <span class="flags">
              {#if m.muted}<span title={t("room.mute")}><Icon name="microphone-off" size={14} /></span>{/if}
              {#if m.deafened}<span title={t("room.deafen")}><Icon name="headphones-off" size={14} /></span>{/if}
            </span>
            {#if m.user_id !== myId}
              <input class="vol" type="range" min="0" max="2" step="0.05"
                value={volumes[m.user_id] ?? 1}
                oninput={(e) => setVolume(m.user_id, Number((e.target as HTMLInputElement).value))}
                title={t("room.volume", { pct: Math.round((volumes[m.user_id] ?? 1) * 100) })} />
            {/if}
          </li>
        {/each}
      </ul>
    </section>

    <section class="bar">
      {@render meter()}
      <button class="iconbtn" class:active={muted} onclick={toggleMute} title={t("room.mute")} aria-label={t("room.mute")} aria-pressed={muted}>
        <Icon name={muted ? "microphone-off" : "microphone"} />
      </button>
      <button class="iconbtn" class:active={deafened} onclick={toggleDeafen} title={t("room.deafen")} aria-label={t("room.deafen")} aria-pressed={deafened}>
        <Icon name={deafened ? "headphones-off" : "headphones"} />
      </button>
      <span class="stats">{t("room.stats", { rtt: stats.rtt_ms, jitter: stats.jitter_ms })}{stats.lost ? ` · ${t("room.lost", { n: stats.lost })}` : ""}</span>
    </section>
    {@render inputHint()}
    {#if settings.push_to_talk}
      <p class="hint">{t("room.pttHint", { key: prettyShortcut(settings.ptt_shortcut) })}</p>
    {/if}
  {/if}

  <footer>v{version}{me?.muted ? ` · ${t("footer.muted")}` : ""}</footer>
</main>
