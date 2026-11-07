// English: the source language. Every other dictionary must provide exactly these keys
// (enforced by the `Messages` type). Placeholders look like `{name}`.
export const en = {
  // header
  "header.connected": "connected · {ms} ms",
  "header.connecting": "connecting…",
  "header.reconnecting": "reconnecting…",
  "header.settings": "Settings",

  "update.available": "CallOut {version} is available.",
  "update.availablePortable": "CallOut {version} is available. Download the new callout-app.exe and replace this one.",
  "update.download": "Download",
  "update.install": "Update and restart",
  "update.downloading": "Downloading the update… {pct}%",
  "update.failed": "The update failed. Download it from the Releases page.",

  // settings
  "settings.title": "Settings",
  "settings.language": "Language",
  "settings.languageAuto": "System default",
  "settings.microphone": "Microphone",
  "settings.speaker": "Audio output",
  "settings.default": "Default",
  "settings.noiseSuppression": "Noise suppression (fans, hum, room noise)",
  "settings.agc": "Automatic volume (evens out quiet and loud speech)",
  "settings.joinSound": "Join sound",
  "settings.joinSoundPlay": "Listen (join, then leave)",
  "joinSound.chime": "Chime",
  "joinSound.pop": "Bubble",
  "joinSound.marimba": "Marimba",
  "joinSound.beep": "Radio",
  "joinSound.off": "Off",
  "settings.pushToTalk": "Push-to-talk (otherwise voice activity)",
  "settings.sensitivity": "Voice activity sensitivity",
  "settings.sensitivityLow": "Less",
  "settings.sensitivityHigh": "More",
  "settings.sensitivityReset": "Reset to default",
  "settings.pttKey": "Push-to-talk key",
  "settings.pttCapturing": "Press a key, combination or mouse button… (Esc to cancel)",
  "settings.pttUnassigned": "Not set — click, then press a key or mouse button",
  "settings.pttHint":
    "Active only while you are in a room. CallOut doesn't block the key, so it still works in your game too: pick one the game doesn't use, or a mouse side button. If the game runs as administrator, run CallOut as administrator too.",
  "settings.save": "Save",
  "settings.cancel": "Cancel",

  // sound test
  "test.title": "Sound test",
  "test.threshold": "Voice activity threshold",
  "test.calibrating": "Getting ready… stay quiet",
  "test.recording": "Speak… {s} s",
  "test.playing": "Playing recording… (stop)",
  "test.start": "Test microphone",
  "test.tone": "Play test sound",
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
  "lobby.forget": "Remove from the list",
  "lobby.disconnect": "Disconnect",

  // room
  "room.copyCode": "Copy code",
  "room.leave": "Leave",
  "room.close": "Close room",
  "room.closeAsk": "Close the room for everyone? It is deleted from the server.",
  "room.closeHint": "Closes the room for everyone and deletes it from the server",
  "room.you": "(you)",
  "room.volume": "Volume: {pct}%",
  "room.volumePct": "{pct}%",
  "room.volumeReset": "Reset to 100%",
  "room.mute": "Mute microphone",
  "room.deafen": "Mute sound",
  "room.stats": "{rtt} ms · buffer {jitter} ms",
  "room.lost": "lost {n}",
  "room.pttHint": "Push-to-talk: sending while {key} is held.",
  "room.pttNoKey": "Push-to-talk is on but no key is set; pick one in Settings.",
  "footer.muted": "muted",
  "logs.link": "Send log",
  "logs.ask": "Send CallOut's diagnostic log to the operator of this server? It holds connection quality, audio levels and errors (no audio, no conversations) and is kept for 30 days.",
  "logs.askProblem": "Something went wrong. Send CallOut's diagnostic log to the operator of this server so it can be looked into? It holds connection quality, audio levels and errors (no audio, no conversations) and is kept for 30 days.",
  "logs.send": "Send",
  "logs.notNow": "Not now",
  "logs.sending": "Sending…",
  "logs.stored": "Log sent. Thank you!",
  "logs.unsupported": "This server can't receive logs.",
  "logs.rateLimited": "A log was sent a few minutes ago; try again later.",
  "logs.failed": "Could not send the log: {detail}",
  "logs.close": "Close",

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
  "error.not_owner": "Only the person who created the room can close it.",
  "error.room_closed": "The room was closed by its owner.",
};

export type MessageKey = keyof typeof en;
export type Messages = Record<MessageKey, string>;
