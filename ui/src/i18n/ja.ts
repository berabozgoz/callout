import type { Messages } from "./en";

export const ja: Messages = {
  "header.connected": "接続済み · {ms} ms",
  "header.connecting": "接続中…",
  "header.reconnecting": "再接続中…",
  "header.settings": "設定",

  "update.available": "CallOut {version} が利用可能です。",
  "update.availablePortable": "CallOut {version} が利用可能です。新しい callout-app.exe をダウンロードして、このファイルと置き換えてください。",
  "update.download": "ダウンロード",
  "update.install": "更新して再起動",
  "update.downloading": "アップデートをダウンロードしています… {pct}%",
  "update.failed": "アップデートに失敗しました。Releases ページからダウンロードしてください。",

  "settings.title": "設定",
  "settings.language": "言語",
  "settings.languageAuto": "システムの言語",
  "settings.microphone": "マイク",
  "settings.speaker": "音声出力",
  "settings.default": "既定",
  "settings.noiseSuppression": "ノイズ抑制（ファン、ハム音、室内の雑音）",
  "settings.pushToTalk": "プッシュトゥトーク（オフ時は音声検出）",
  "settings.sensitivity": "音声検出の感度",
  "settings.sensitivityLow": "低",
  "settings.sensitivityHigh": "高",
  "settings.sensitivityReset": "既定に戻す",
  "settings.pttKey": "プッシュトゥトークのキー",
  "settings.pttCapturing": "キーまたは組み合わせを押してください…（Esc でキャンセル）",
  "settings.pttUnassigned": "未設定 — クリックしてキーを押してください",
  "settings.pttHint":
    "ショートカットはルームにいる間だけ有効です。単一の文字キー（例：T）を選ぶと、ルームにいる間はほかのアプリでその文字を入力できなくなります。F9 や Ctrl+Alt などの組み合わせのほうが安全です。",
  "settings.save": "保存",
  "settings.cancel": "キャンセル",

  "test.title": "サウンドテスト",
  "test.threshold": "音声検出のしきい値",
  "test.calibrating": "準備中…静かにしてください",
  "test.recording": "話してください… {s} 秒",
  "test.playing": "録音を再生中…（停止）",
  "test.start": "マイクをテスト",
  "test.tone": "テスト音を再生",
  "test.resultNone":
    "録音のうち送信される部分はありませんでした。マイクの音量がしきい値を下回っていたか、別のマイクが選択されています。",
  "test.result":
    "録音の {pct}% が音声として送信され、残りは無音と判定されました。言葉の始めや終わりが切れていた場合は、声がしきい値にかなり近い状態です。",
  "test.hintPtt":
    "5 秒間話してください。録音はルームのほかの人に聞こえるとおりに再生されます。プッシュトゥトークがオンでも、テストではキーを押さずに録音します。",
  "test.hintVad":
    "5 秒間話してください。録音はルームのほかの人に聞こえるとおりに再生されます。線は音声検出のしきい値で、バーが線を超えると声が送信されます（緑）。",

  "connect.title": "サーバーに接続",
  "connect.nickname": "ニックネーム",
  "connect.nicknamePlaceholder": "ユーザー名",
  "connect.invite": "招待リンク",
  "connect.invitePlaceholder": "callout://IP:4433#フィンガープリント",
  "connect.button": "接続",
  "connect.connecting": "接続中…",
  "connect.hint":
    "招待リンクはサーバーを運用している友だちから受け取ってください。サーバーは初回起動時にリンクを表示します。",
  "connect.nicknameRequired": "ニックネームを入力してください",
  "connect.inviteInvalid": "招待リンクは callout:// で始まる必要があります",

  "reconnect.title": "再接続しています…",
  "reconnect.hint": "サーバーとの接続が切れました。CallOut は接続を試み続け、元のルームに戻します。",
  "reconnect.cancel": "停止",

  "lobby.create": "ルームを作成",
  "lobby.roomName": "ルーム名（任意）",
  "lobby.defaultRoomName": "{name}のルーム",
  "lobby.createButton": "ルームを作成",
  "lobby.join": "ルームに参加",
  "lobby.code": "ルームコード",
  "lobby.joinButton": "参加",
  "lobby.forget": "リストから削除",
  "lobby.disconnect": "切断",

  "room.copyCode": "コードをコピー",
  "room.leave": "退出",
  "room.close": "ルームを閉じる",
  "room.closeConfirm": "もう一度クリックで閉じる",
  "room.closeHint": "全員のルームを閉じてサーバーから削除します",
  "room.you": "（自分）",
  "room.volume": "音量：{pct}%",
  "room.mute": "マイクをミュート",
  "room.deafen": "スピーカーをミュート",
  "room.stats": "{rtt} ms · バッファー {jitter} ms",
  "room.lost": "ロス {n}",
  "room.pttHint": "プッシュトゥトーク：{key} を押している間だけ送信します。",
  "footer.muted": "ミュート中",

  "tray.show": "CallOut を表示",
  "tray.quit": "終了",

  "audio.fallback": "選択したオーディオデバイスが見つからないため、既定のデバイスに切り替えました。",
  "audio.listenOnly": "聞くだけ：使用できるマイクがないため、他の人にはあなたの声が聞こえません。",
  "audio.bluetoothMic":
    "マイクが {khz} kHz で動作しています。これは通常、Bluetooth ヘッドセットがハンズフリー モードになっていることを意味し、音質が下がり送信も不安定になります。有線または USB マイクのほうがずっと良く聞こえます。Bluetooth ヘッドセットは聞く用途に使い続けられます。",

  "error.generic": "{detail}",
  "error.invalid_invite": "招待リンクが無効です。すべてコピーされているか確認してください。",
  "error.device_key": "このデバイスの ID キーを作成できませんでした（{detail}）。",
  "error.connect_failed":
    "サーバーに接続できませんでした。招待リンクと、サーバーが起動しているかを確認してください。（{detail}）",
  "error.audio_failed": "オーディオデバイスを開けませんでした（{detail}）。",
  "error.mic_access_denied":
    "Windows がマイクへのアクセスをブロックしています。「設定 → プライバシー → マイク」を開き、「デスクトップ アプリがマイクにアクセスできるようにする」をオンにしてから、CallOut を再起動してください。",
  "error.audio_device_lost": "オーディオデバイスが切断されました。自動的に再接続しています…",
  "error.connection_lost": "サーバーとの接続が切れました（{detail}）。",
  "error.request_failed": "リクエストに失敗しました（{detail}）。",
  "error.version_mismatch":
    "この CallOut のバージョンはサーバーと互換性がありません。CallOut またはサーバーを更新してください。",
  "error.room_not_found": "そのコードのルームはありません。",
  "error.room_full": "ルームは満員です。",
  "error.already_in_room": "先に現在のルームから退出してください。",
  "error.not_in_room": "ルームに参加していません。",
  "error.rate_limited": "リクエストが多すぎます。しばらく待ってから、もう一度お試しください。",
  "error.server_error": "サーバーエラー（{detail}）。",
  "error.not_owner": "ルームを閉じられるのは作成した人だけです。",
  "error.room_closed": "ルームは作成者によって閉じられました。",
};
