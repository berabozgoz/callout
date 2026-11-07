<script lang="ts" module>
  // Outline icons from Tabler Icons 3.19 (MIT License, Copyright (c) 2020-2024 Paweł Kuna,
  // https://tabler.io/icons). Inlined so they look the same on every Windows version, unlike emoji.
  const ICONS = {
    microphone: [
      "M9 2m0 3a3 3 0 0 1 3 -3h0a3 3 0 0 1 3 3v5a3 3 0 0 1 -3 3h0a3 3 0 0 1 -3 -3z",
      "M5 10a7 7 0 0 0 14 0",
      "M8 21l8 0",
      "M12 17l0 4",
    ],
    bell: [
      "M10 5a2 2 0 1 1 4 0a7 7 0 0 1 4 6v3a4 4 0 0 0 2 3h-16a4 4 0 0 0 2 -3v-3a7 7 0 0 1 4 -6",
      "M9 17v1a3 3 0 0 0 6 0v-1",
    ],
    "player-play": ["M7 4v16l13 -8z"],
    "microphone-off": [
      "M3 3l18 18",
      "M9 5a3 3 0 0 1 6 0v5a3 3 0 0 1 -.13 .874m-2 2a3 3 0 0 1 -3.87 -2.872v-1",
      "M5 10a7 7 0 0 0 10.846 5.85m2 -2a6.967 6.967 0 0 0 1.152 -3.85",
      "M8 21l8 0",
      "M12 17l0 4",
    ],
    headphones: [
      "M4 13m0 2a2 2 0 0 1 2 -2h1a2 2 0 0 1 2 2v3a2 2 0 0 1 -2 2h-1a2 2 0 0 1 -2 -2z",
      "M15 13m0 2a2 2 0 0 1 2 -2h1a2 2 0 0 1 2 2v3a2 2 0 0 1 -2 2h-1a2 2 0 0 1 -2 -2z",
      "M4 15v-3a8 8 0 0 1 16 0v3",
    ],
    "headphones-off": [
      "M3 3l18 18",
      "M4 13m0 2a2 2 0 0 1 2 -2h1a2 2 0 0 1 2 2v3a2 2 0 0 1 -2 2h-1a2 2 0 0 1 -2 -2z",
      "M17 13h1a2 2 0 0 1 2 2v1m-.589 3.417c-.361 .36 -.86 .583 -1.411 .583h-1a2 2 0 0 1 -2 -2v-3",
      "M4 15v-3c0 -2.21 .896 -4.21 2.344 -5.658m2.369 -1.638a8 8 0 0 1 11.287 7.296v3",
    ],
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each ICONS[name] as d (d)}<path {d} />{/each}
</svg>
