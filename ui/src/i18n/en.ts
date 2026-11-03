// English: the source language. Every other dictionary must provide exactly these keys
// (enforced by the `Messages` type). Placeholders look like `{name}`.
export const en = {
  // header
  "header.connected": "connected · {ms} ms",
  "header.connecting": "connecting…",
  "header.reconnecting": "reconnecting…",
  "header.settings": "Settings",

  // settings
  "settings.title": "Settings",
  "settings.language": "Language",
  "settings.languageAuto": "System default",
  "settings.microphone": "Microphone",
  "settings.speaker": "Speaker",
  "settings.default": "Default",
  "settings.noiseSuppression": "Noise suppression (fans, hum, room noise)",
  "settings.pushToTalk": "Push-to-talk (otherwise voice activity)",
  "settings.sensitivity": "Voice activity sensitivity",
  "settings.sensitivityLow": "Less",
  "settings.sensitivityHigh": "More",
  "settings.pttKey": "Push-to-talk key",
  "settings.pttCapturing": "Press a key or combination… (Esc to cancel)",
  "settings.pttUnassigned": "Not set — click and press a key",
  "settings.pttHint":
    "The shortcut is only active while you are in a room. If you pick a plain letter (e.g. T), you can't type that letter in other apps while in a room; F9 or combinations like Ctrl+Alt are safer.",
  "settings.save": "Save",
  "settings.cancel": "Cancel",

  // sound test
  "test.title": "Sound test",
  "test.threshold": "Voice activity threshold",
  "test.calibrating": "Getting ready… stay quiet",
  "test.recording": "⏺ Speak… {s} s",
  "test.playing": "▶ Playing recording… (stop)",
  "test.start": "🎙 Test microphone",
  "test.tone": "🔊 Play test sound",
  "test.resultNone":
    "Nothing in the recording would have been sent: your microphone stayed below the threshold, or the wrong microphone is selected.",
  "test.result":
    "{pct}% of the recording would have been sent as voice; the rest counted as silence. If the start or end of your words was cut off, your voice is very close to the threshold.",
  "test.hintPtt":
    "Speak for 5 s; the recording plays back the way others in the room will hear it. With push-to-talk on, the test records without the key.",
  "test.hintVad":
    "Speak for 5 s; the recording plays back the way others in the room will hear it. The line is the voice activity threshold: when the bar crosses it, your voice is sent (green).",

  // connect
  "connect.title": "Connect to a server",
  "connect.nickname": "Nickname",
  "connect.nicknamePlaceholder": "username",
  "connect.invite": "Invite link",
  "connect.invitePlaceholder": "callout://IP:4433#fingerprint",
  "connect.button": "Connect",
  "connect.connecting": "Connecting…",
  "connect.hint":
    "Get the invite link from the friend who runs the server. The server prints the link when it first starts.",
  "connect.nicknameRequired": "A nickname is required",
  "connect.inviteInvalid": "The invite link must start with callout://",

  "reconnect.title": "Reconnecting…",
  "reconnect.hint": "The connection to the server was lost. CallOut keeps trying and puts you back in your room.",
  "reconnect.cancel": "Stop",

  // lobby
  "lobby.create": "Create a room",
  "lobby.roomName": "Room name (optional)",
  // used when the room name is left empty; the creator's language names the room
  "lobby.defaultRoomName": "{name}'s room",
  "lobby.createButton": "Create room",
  "lobby.join": "Join a room",
  "lobby.code": "Room code",
  "lobby.joinButton": "Join",
  "lobby.disconnect": "Disconnect",

  // room
  "room.copyCode": "Copy code",
  "room.leave": "Leave",
  "room.you": "(you)",
  "room.volume": "Volume: {pct}%",
  "room.mute": "Mute microphone",
  "room.deafen": "Mute sound",
  "room.stats": "{rtt} ms · buffer {jitter} ms",
  "room.lost": "lost {n}",
  "room.pttHint": "Push-to-talk: sending while {key} is held.",
  "footer.muted": "muted",

  // tray menu (sent to the app shell)
  "tray.show": "Show CallOut",
  "tray.quit": "Quit",

  "audio.fallback": "The selected audio device was not found; switched to the default device.",
  "audio.listenOnly": "Listening only: no microphone is available, so others can't hear you.",
  "audio.bluetoothMic":
    "Your microphone runs at {khz} kHz. That usually means a Bluetooth headset in hands-free mode: lower quality and uneven timing. A wired or USB mic sounds much better; the Bluetooth headset can stay for listening.",

  // engine / server errors (Event::Error kind → text; {detail} is the technical cause)
  "error.generic": "{detail}",
  "error.invalid_invite": "The invite link is invalid. Make sure you copied all of it.",
  "error.device_key": "Could not create this device's identity key ({detail}).",
  "error.connect_failed": "Could not connect to the server. Check the invite link and that the server is running. ({detail})",
  "error.audio_failed": "Could not open the audio devices ({detail}).",
  "error.mic_access_denied":
    "Windows is blocking microphone access. Open Settings → Privacy → Microphone, turn on “Let desktop apps access your microphone”, then restart CallOut.",
  "error.audio_device_lost": "Audio device lost; reconnecting automatically…",
  "error.connection_lost": "The connection to the server was lost ({detail}).",
  "error.request_failed": "The request failed ({detail}).",
  "error.version_mismatch": "This version of CallOut doesn't match the server. Update CallOut or the server.",
  "error.room_not_found": "There is no room with that code.",
  "error.room_full": "The room is full.",
  "error.already_in_room": "Leave your current room first.",
  "error.not_in_room": "You are not in a room.",
  "error.rate_limited": "Too many requests; wait a moment and try again.",
  "error.server_error": "Server error ({detail}).",
};

export type MessageKey = keyof typeof en;
export type Messages = Record<MessageKey, string>;
