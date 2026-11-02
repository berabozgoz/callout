// Engine bridge: types and helpers for the Tauri commands and the "engine" event channel.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type RoomInfo = { code: string; name: string; owner: number };
export type Member = { user_id: number; nickname: string; muted: boolean; deafened: boolean };
export type ConnectionState = "disconnected" | "connecting" | "connected";
export type MicTestState = "calibrating" | "recording" | "playing" | "idle";

export type EngineEvent =
  | { type: "connection_state"; state: ConnectionState }
  | { type: "connected"; user_id: number }
  | { type: "room_updated"; room: RoomInfo | null; members: Member[] }
  | { type: "speaking_changed"; user_id: number; speaking: boolean }
  | { type: "input_level"; level: number; transmitting: boolean; threshold: number }
  | { type: "stats"; rtt_ms: number; jitter_ms: number; lost: number; sent: number; received: number }
  | { type: "audio_restored"; fallback_to_default: boolean }
  | { type: "mic_test"; state: MicTestState; transmitted_pct: number | null }
  | { type: "error"; message: string };

export type Command =
  | { type: "connect"; invite: string; nickname: string }
  | { type: "disconnect" }
  | { type: "create_room"; name: string }
  | { type: "join_room"; code: string }
  | { type: "leave_room" }
  | { type: "set_muted"; muted: boolean }
  | { type: "set_deafened"; deafened: boolean }
  | { type: "set_push_to_talk_mode"; enabled: boolean }
  | { type: "push_to_talk"; pressed: boolean }
  | { type: "set_input_device"; name: string | null }
  | { type: "set_output_device"; name: string | null }
  | { type: "set_user_volume"; user_id: number; gain: number }
  | { type: "set_audio_preview"; enabled: boolean }
  | { type: "start_mic_test" }
  | { type: "stop_mic_test" }
  | { type: "play_test_sound" }
  | { type: "set_noise_suppression"; enabled: boolean };

export type Settings = {
  nickname: string;
  last_invite: string;
  input_device: string | null;
  output_device: string | null;
  push_to_talk: boolean;
  ptt_shortcut: string;
  noise_suppression: boolean;
};

/** The engine's actual state: the engine stays connected even if the webview reloads. */
export type Snapshot = {
  connection: ConnectionState;
  user_id: number | null;
  room: RoomInfo | null;
  members: Member[];
  muted: boolean;
  deafened: boolean;
};

export type Device = { name: string; direction: "input" | "output"; is_default: boolean };

export const send = (command: Command) => invoke<void>("engine_command", { command });
export const getSettings = () => invoke<Settings>("get_settings");
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });
export const engineSnapshot = () => invoke<Snapshot>("engine_snapshot");
/** Temporarily releases the push-to-talk shortcut during key capture (otherwise the key never reaches the UI). */
export const pttCapture = (active: boolean) => invoke<void>("ptt_capture", { active });
export const listDevices = () => invoke<Device[]>("list_audio_devices");
export const appVersion = () => invoke<string>("app_version");

export function onEngine(handler: (e: EngineEvent) => void): Promise<UnlistenFn> {
  return listen<EngineEvent>("engine", (ev) => handler(ev.payload));
}

export function onPtt(handler: (pressed: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("ptt", (ev) => handler(ev.payload));
}

/** `Ctrl+KeyT` → `Ctrl + T`, `Alt+ArrowUp` → `Alt + ↑` */
export function prettyShortcut(s: string): string {
  const arrows: Record<string, string> = { ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→" };
  return s
    .split("+")
    .filter(Boolean)
    .map((p) => arrows[p] ?? p.replace(/^Key/, "").replace(/^Digit/, ""))
    .join(" + ");
}

/** `ABCD1234` → `ABCD-1234` */
export const prettyCode = (code: string) => (code.length === 8 ? `${code.slice(0, 4)}-${code.slice(4)}` : code);
