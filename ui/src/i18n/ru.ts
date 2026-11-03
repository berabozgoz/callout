import type { Messages } from "./en";

export const ru: Messages = {
  "header.connected": "подключено · {ms} ms",
  "header.connecting": "подключение…",
  "header.reconnecting": "переподключение…",
  "header.settings": "Настройки",

  "update.available": "Доступна версия CallOut {version}.",
  "update.availablePortable": "Доступна версия CallOut {version}. Скачайте новый callout-app.exe и замените этот файл.",
  "update.download": "Скачать",
  "update.install": "Обновить и перезапустить",
  "update.downloading": "Загрузка обновления… {pct} %",
  "update.failed": "Не удалось обновить. Скачайте новую версию на странице Releases.",

  "settings.title": "Настройки",
  "settings.language": "Язык",
  "settings.languageAuto": "Как в системе",
  "settings.microphone": "Микрофон",
  "settings.speaker": "Динамики",
  "settings.default": "По умолчанию",
  "settings.noiseSuppression": "Шумоподавление (вентиляторы, гул, шум комнаты)",
  "settings.pushToTalk": "Режим рации (иначе активация по голосу)",
  "settings.sensitivity": "Чувствительность активации по голосу",
  "settings.sensitivityLow": "Меньше",
  "settings.sensitivityHigh": "Больше",
  "settings.pttKey": "Клавиша рации",
  "settings.pttCapturing": "Нажмите клавишу или сочетание… (Esc — отмена)",
  "settings.pttUnassigned": "Не назначена — нажмите здесь, затем клавишу",
  "settings.pttHint":
    "Сочетание работает, только пока вы в комнате. Если выбрать обычную букву (например, T), в комнате её нельзя будет набрать в других приложениях; F9 или сочетания вроде Ctrl+Alt надёжнее.",
  "settings.save": "Сохранить",
  "settings.cancel": "Отмена",

  "test.title": "Проверка звука",
  "test.threshold": "Порог активации по голосу",
  "test.calibrating": "Подготовка… помолчите",
  "test.recording": "⏺ Говорите… {s} с",
  "test.playing": "▶ Воспроизведение записи… (стоп)",
  "test.start": "🎙 Проверить микрофон",
  "test.tone": "🔊 Тестовый звук",
  "test.resultNone":
    "Из записи ничего не было бы отправлено: микрофон не превысил порог или выбран не тот микрофон.",
  "test.result":
    "{pct} % записи было бы отправлено как голос, остальное считалось тишиной. Если начало или конец слов обрезались, ваш голос очень близок к порогу.",
  "test.hintPtt":
    "Говорите 5 с; запись воспроизведётся так, как вас услышат остальные в комнате. В режиме рации проверка записывает без нажатия клавиши.",
  "test.hintVad":
    "Говорите 5 с; запись воспроизведётся так, как вас услышат остальные в комнате. Линия — порог активации по голосу: когда полоска её пересекает, голос отправляется (зелёный цвет).",

  "connect.title": "Подключение к серверу",
  "connect.nickname": "Никнейм",
  "connect.nicknamePlaceholder": "имя пользователя",
  "connect.invite": "Ссылка-приглашение",
  "connect.invitePlaceholder": "callout://IP:4433#отпечаток",
  "connect.button": "Подключиться",
  "connect.connecting": "Подключение…",
  "connect.hint":
    "Ссылку-приглашение даст друг, у которого запущен сервер. Сервер выводит ссылку при первом запуске.",
  "connect.nicknameRequired": "Укажите никнейм",
  "connect.inviteInvalid": "Ссылка-приглашение должна начинаться с callout://",

  "reconnect.title": "Переподключение…",
  "reconnect.hint": "Соединение с сервером потеряно. CallOut продолжает попытки и вернёт вас в вашу комнату.",
  "reconnect.cancel": "Остановить",

  "lobby.create": "Создать комнату",
  "lobby.roomName": "Название комнаты (необязательно)",
  "lobby.defaultRoomName": "Комната {name}",
  "lobby.createButton": "Создать",
  "lobby.join": "Войти в комнату",
  "lobby.code": "Код комнаты",
  "lobby.joinButton": "Войти",
  "lobby.forget": "Убрать из списка",
  "lobby.disconnect": "Отключиться",

  "room.copyCode": "Копировать код",
  "room.leave": "Выйти",
  "room.you": "(вы)",
  "room.volume": "Громкость: {pct} %",
  "room.mute": "Выключить микрофон",
  "room.deafen": "Выключить звук",
  "room.stats": "{rtt} ms · буфер {jitter} ms",
  "room.lost": "потеряно: {n}",
  "room.pttHint": "Режим рации: голос передаётся, пока зажата клавиша {key}.",
  "footer.muted": "микрофон выкл.",

  "tray.show": "Показать CallOut",
  "tray.quit": "Выход",

  "audio.fallback": "Выбранное аудиоустройство не найдено; используется устройство по умолчанию.",
  "audio.listenOnly": "Только прослушивание: микрофон недоступен, другие вас не слышат.",
  "audio.bluetoothMic":
    "Ваш микрофон работает на {khz} кГц. Обычно это Bluetooth-гарнитура в режиме громкой связи: качество ниже, а пакеты идут неравномерно. Проводной или USB-микрофон звучит гораздо лучше; Bluetooth-гарнитуру можно оставить для прослушивания.",

  "error.generic": "{detail}",
  "error.invalid_invite": "Неверная ссылка-приглашение. Убедитесь, что скопировали её целиком.",
  "error.device_key": "Не удалось создать ключ идентификации этого устройства ({detail}).",
  "error.connect_failed": "Не удалось подключиться к серверу. Проверьте ссылку-приглашение и что сервер запущен. ({detail})",
  "error.audio_failed": "Не удалось открыть аудиоустройства ({detail}).",
  "error.mic_access_denied":
    "Windows блокирует доступ к микрофону. Откройте Параметры → Конфиденциальность → Микрофон, включите «Разрешить классическим приложениям доступ к микрофону» и перезапустите CallOut.",
  "error.audio_device_lost": "Аудиоустройство потеряно; автоматическое переподключение…",
  "error.connection_lost": "Соединение с сервером потеряно ({detail}).",
  "error.request_failed": "Запрос не выполнен ({detail}).",
  "error.version_mismatch": "Эта версия CallOut несовместима с сервером. Обновите CallOut или сервер.",
  "error.room_not_found": "Комнаты с таким кодом нет.",
  "error.room_full": "Комната заполнена.",
  "error.already_in_room": "Сначала выйдите из текущей комнаты.",
  "error.not_in_room": "Вы не в комнате.",
  "error.rate_limited": "Слишком много запросов; подождите немного и повторите.",
  "error.server_error": "Ошибка сервера ({detail}).",
};
