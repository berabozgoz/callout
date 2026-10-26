<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type Device = {
    name: string;
    direction: "input" | "output";
    is_default: boolean;
    sample_rate_hz: number;
    channels: number;
  };

  let devices = $state<Device[]>([]);
  let version = $state("");
  let error = $state("");

  onMount(async () => {
    try {
      version = await invoke<string>("app_version");
      devices = await invoke<Device[]>("list_audio_devices");
    } catch (e) {
      error = String(e);
    }
  });

  const inputs = $derived(devices.filter((d) => d.direction === "input"));
  const outputs = $derived(devices.filter((d) => d.direction === "output"));
</script>

<main>
  <h1>CallOut</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <h2>Mikrofonlar</h2>
  <ul>
    {#each inputs as d (d.name)}
      <li class:default={d.is_default}>
        <span>{d.name}</span>
        <small>{d.sample_rate_hz / 1000} kHz · {d.channels} ch</small>
      </li>
    {:else}
      <li><span>Giriş cihazı bulunamadı</span></li>
    {/each}
  </ul>

  <h2>Hoparlörler</h2>
  <ul>
    {#each outputs as d (d.name)}
      <li class:default={d.is_default}>
        <span>{d.name}</span>
        <small>{d.sample_rate_hz / 1000} kHz · {d.channels} ch</small>
      </li>
    {:else}
      <li><span>Çıkış cihazı bulunamadı</span></li>
    {/each}
  </ul>

  <footer>Faz 0 iskeleti · v{version}</footer>
</main>
