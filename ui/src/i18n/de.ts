import type { Messages } from "./en";

export const de: Messages = {
  "header.connected": "verbunden · {ms} ms",
  "header.connecting": "wird verbunden…",
  "header.settings": "Einstellungen",

  "settings.title": "Einstellungen",
  "settings.language": "Sprache",
  "settings.languageAuto": "Systemsprache",
  "settings.microphone": "Mikrofon",
  "settings.speaker": "Lautsprecher",
  "settings.default": "Standard",
  "settings.noiseSuppression": "Rauschunterdrückung (Lüfter, Brummen, Raumgeräusche)",
  "settings.pushToTalk": "Push-to-Talk (sonst Sprachaktivierung)",
  "settings.pttKey": "Push-to-Talk-Taste",
  "settings.pttCapturing": "Drücke eine Taste oder Kombination… (Esc zum Abbrechen)",
  "settings.pttUnassigned": "Nicht festgelegt — klicken und eine Taste drücken",
  "settings.pttHint":
    "Das Tastenkürzel ist nur aktiv, solange du in einem Raum bist. Wenn du einen einfachen Buchstaben (z. B. T) wählst, kannst du diesen Buchstaben in anderen Apps nicht tippen, solange du in einem Raum bist; F9 oder Kombinationen wie Strg+Alt sind sicherer.",
  "settings.save": "Speichern",
  "settings.cancel": "Abbrechen",

  "test.title": "Soundtest",
  "test.threshold": "Schwellenwert der Sprachaktivierung",
  "test.calibrating": "Wird vorbereitet… bitte still sein",
  "test.recording": "⏺ Sprich… {s} s",
  "test.playing": "▶ Aufnahme wird abgespielt… (Stopp)",
  "test.start": "🎙 Mikrofon testen",
  "test.tone": "🔊 Testton abspielen",
  "test.resultNone":
    "In der Aufnahme wäre nichts gesendet worden: Dein Mikrofon blieb unter dem Schwellenwert, oder das falsche Mikrofon ist ausgewählt.",
  "test.result":
    "{pct} % der Aufnahme wären als Sprache gesendet worden; der Rest galt als Stille. Wenn Anfang oder Ende deiner Wörter abgeschnitten wurden, liegt deine Stimme sehr nah am Schwellenwert.",
  "test.hintPtt":
    "Sprich 5 s lang; die Aufnahme wird so abgespielt, wie andere im Raum dich hören werden. Bei aktiviertem Push-to-Talk nimmt der Test auch ohne Taste auf.",
  "test.hintVad":
    "Sprich 5 s lang; die Aufnahme wird so abgespielt, wie andere im Raum dich hören werden. Die Linie ist der Schwellenwert der Sprachaktivierung: Überschreitet der Balken sie, wird deine Stimme gesendet (grün).",

  "connect.title": "Mit einem Server verbinden",
  "connect.nickname": "Spitzname",
  "connect.invite": "Einladungslink",
  "connect.invitePlaceholder": "callout://IP:4433#Fingerabdruck",
  "connect.button": "Verbinden",
  "connect.connecting": "Wird verbunden…",
  "connect.hint":
    "Den Einladungslink bekommst du von der Person, die den Server betreibt. Der Server gibt den Link beim ersten Start aus.",
  "connect.nicknameRequired": "Ein Spitzname ist erforderlich",
  "connect.inviteInvalid": "Der Einladungslink muss mit callout:// beginnen",

  "lobby.create": "Raum erstellen",
  "lobby.roomName": "Raumname (optional)",
  "lobby.roomNamePlaceholder": "Team",
  "lobby.createButton": "Raum erstellen",
  "lobby.join": "Raum beitreten",
  "lobby.code": "Raumcode",
  "lobby.joinButton": "Beitreten",
  "lobby.disconnect": "Trennen",

  "room.copyCode": "Code kopieren",
  "room.leave": "Verlassen",
  "room.you": "(du)",
  "room.volume": "Lautstärke: {pct} %",
  "room.mute": "Mikrofon stummschalten",
  "room.deafen": "Ton stummschalten",
  "room.stats": "{rtt} ms · Puffer {jitter} ms",
  "room.lost": "{n} verloren",
  "room.pttHint": "Push-to-Talk: Gesendet wird, solange {key} gedrückt ist.",
  "footer.muted": "stumm",

  "tray.show": "CallOut anzeigen",
  "tray.quit": "Beenden",

  "audio.fallback": "Das ausgewählte Audiogerät wurde nicht gefunden; stattdessen wird das Standardgerät verwendet.",

  "error.generic": "{detail}",
  "error.invalid_invite": "Der Einladungslink ist ungültig. Achte darauf, dass du ihn vollständig kopiert hast.",
  "error.device_key": "Der Identitätsschlüssel dieses Geräts konnte nicht erstellt werden ({detail}).",
  "error.connect_failed":
    "Verbindung zum Server fehlgeschlagen. Prüfe den Einladungslink und ob der Server läuft. ({detail})",
  "error.audio_failed": "Die Audiogeräte konnten nicht geöffnet werden ({detail}).",
  "error.mic_access_denied":
    "Windows blockiert den Mikrofonzugriff. Öffne Einstellungen → Datenschutz → Mikrofon, aktiviere „Desktop-Apps den Zugriff auf Ihr Mikrofon erlauben“ und starte CallOut anschließend neu.",
  "error.audio_device_lost": "Audiogerät verloren; wird automatisch wieder verbunden…",
  "error.connection_lost": "Die Verbindung zum Server wurde unterbrochen ({detail}).",
  "error.request_failed": "Die Anfrage ist fehlgeschlagen ({detail}).",
  "error.version_mismatch": "Diese CallOut-Version passt nicht zum Server. Aktualisiere CallOut oder den Server.",
  "error.room_not_found": "Es gibt keinen Raum mit diesem Code.",
  "error.room_full": "Der Raum ist voll.",
  "error.already_in_room": "Verlasse zuerst deinen aktuellen Raum.",
  "error.not_in_room": "Du bist in keinem Raum.",
  "error.rate_limited": "Zu viele Anfragen; warte kurz und versuche es erneut.",
  "error.server_error": "Serverfehler ({detail}).",
};
