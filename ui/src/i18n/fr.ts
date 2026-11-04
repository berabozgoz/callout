import type { Messages } from "./en";

export const fr: Messages = {
  "header.connected": "connecté · {ms} ms",
  "header.connecting": "connexion…",
  "header.reconnecting": "reconnexion…",
  "header.settings": "Paramètres",

  "update.available": "CallOut {version} est disponible.",
  "update.availablePortable": "CallOut {version} est disponible. Téléchargez le nouveau callout-app.exe et remplacez celui-ci.",
  "update.download": "Télécharger",
  "update.install": "Mettre à jour et redémarrer",
  "update.downloading": "Téléchargement de la mise à jour… {pct} %",
  "update.failed": "La mise à jour a échoué. Téléchargez-la depuis la page Releases.",

  "settings.title": "Paramètres",
  "settings.language": "Langue",
  "settings.languageAuto": "Langue du système",
  "settings.microphone": "Microphone",
  "settings.speaker": "Sortie audio",
  "settings.default": "Par défaut",
  "settings.noiseSuppression": "Suppression du bruit (ventilateurs, bourdonnement, bruit ambiant)",
  "settings.pushToTalk": "Appuyer pour parler (sinon détection de la voix)",
  "settings.sensitivity": "Sensibilité de la détection de la voix",
  "settings.sensitivityLow": "Moins",
  "settings.sensitivityHigh": "Plus",
  "settings.sensitivityReset": "Réinitialiser",
  "settings.pttKey": "Touche pour parler",
  "settings.pttCapturing": "Appuyez sur une touche ou une combinaison… (Échap pour annuler)",
  "settings.pttUnassigned": "Non défini — cliquez puis appuyez sur une touche",
  "settings.pttHint":
    "Le raccourci n'est actif que lorsque vous êtes dans un salon. Si vous choisissez une simple lettre (p. ex. T), vous ne pourrez plus taper cette lettre dans d'autres applications tant que vous serez dans un salon ; F9 ou des combinaisons comme Ctrl+Alt sont plus sûres.",
  "settings.save": "Enregistrer",
  "settings.cancel": "Annuler",

  "test.title": "Test audio",
  "test.threshold": "Seuil de détection de la voix",
  "test.calibrating": "Préparation… restez silencieux",
  "test.recording": "Parlez… {s} s",
  "test.playing": "Lecture de l'enregistrement… (arrêter)",
  "test.start": "Tester le micro",
  "test.tone": "Jouer un son de test",
  "test.resultNone":
    "Rien dans l'enregistrement n'aurait été envoyé : votre micro est resté sous le seuil, ou le mauvais micro est sélectionné.",
  "test.result":
    "{pct} % de l'enregistrement aurait été envoyé comme voix ; le reste a été compté comme silence. Si le début ou la fin de vos mots a été coupé, votre voix est très proche du seuil.",
  "test.hintPtt":
    "Parlez pendant 5 s ; l'enregistrement est rejoué tel que les autres membres du salon vous entendront. Avec Appuyer pour parler activé, le test enregistre sans la touche.",
  "test.hintVad":
    "Parlez pendant 5 s ; l'enregistrement est rejoué tel que les autres membres du salon vous entendront. La ligne représente le seuil de détection de la voix : quand la barre la dépasse, votre voix est envoyée (vert).",

  "connect.title": "Se connecter à un serveur",
  "connect.nickname": "Pseudo",
  "connect.nicknamePlaceholder": "nom d’utilisateur",
  "connect.invite": "Lien d'invitation",
  "connect.invitePlaceholder": "callout://IP:4433#empreinte",
  "connect.button": "Se connecter",
  "connect.connecting": "Connexion…",
  "connect.hint":
    "Demandez le lien d'invitation à la personne qui héberge le serveur. Le serveur affiche le lien lors de son premier démarrage.",
  "connect.nicknameRequired": "Un pseudo est requis",
  "connect.inviteInvalid": "Le lien d'invitation doit commencer par callout://",

  "reconnect.title": "Reconnexion…",
  "reconnect.hint": "La connexion au serveur a été perdue. CallOut continue d’essayer et vous remettra dans votre salon.",
  "reconnect.cancel": "Arrêter",

  "lobby.create": "Créer un salon",
  "lobby.roomName": "Nom du salon (facultatif)",
  "lobby.defaultRoomName": "Salon de {name}",
  "lobby.createButton": "Créer le salon",
  "lobby.join": "Rejoindre un salon",
  "lobby.code": "Code du salon",
  "lobby.joinButton": "Rejoindre",
  "lobby.forget": "Retirer de la liste",
  "lobby.disconnect": "Se déconnecter",

  "room.copyCode": "Copier le code",
  "room.leave": "Quitter",
  "room.close": "Fermer le salon",
  "room.closeConfirm": "Cliquez à nouveau pour fermer",
  "room.closeHint": "Ferme le salon pour tout le monde et le supprime du serveur",
  "room.you": "(vous)",
  "room.volume": "Volume : {pct} %",
  "room.mute": "Couper le micro",
  "room.deafen": "Couper le son",
  "room.stats": "{rtt} ms · tampon {jitter} ms",
  "room.lost": "{n} perdus",
  "room.pttHint": "Appuyer pour parler : envoi tant que la touche {key} est maintenue.",
  "footer.muted": "muet",

  "tray.show": "Afficher CallOut",
  "tray.quit": "Quitter",

  "audio.fallback": "Le périphérique audio sélectionné est introuvable ; le périphérique par défaut est utilisé.",
  "audio.listenOnly": "Écoute seule : aucun micro disponible, les autres ne vous entendent pas.",
  "audio.bluetoothMic":
    "Votre micro fonctionne à {khz} kHz. Cela indique généralement un casque Bluetooth en mode mains libres : qualité réduite et envoi irrégulier. Un micro filaire ou USB sonne bien mieux ; le casque Bluetooth peut rester pour l’écoute.",

  "error.generic": "{detail}",
  "error.invalid_invite": "Le lien d'invitation n'est pas valide. Vérifiez que vous l'avez copié en entier.",
  "error.device_key": "Impossible de créer la clé d'identité de cet appareil ({detail}).",
  "error.connect_failed":
    "Impossible de se connecter au serveur. Vérifiez le lien d'invitation et que le serveur est bien démarré. ({detail})",
  "error.audio_failed": "Impossible d'ouvrir les périphériques audio ({detail}).",
  "error.mic_access_denied":
    "Windows bloque l'accès au microphone. Ouvrez Paramètres → Confidentialité → Microphone, activez « Autoriser les applications de bureau à accéder à votre microphone », puis redémarrez CallOut.",
  "error.audio_device_lost": "Périphérique audio perdu ; reconnexion automatique…",
  "error.connection_lost": "La connexion au serveur a été perdue ({detail}).",
  "error.request_failed": "La requête a échoué ({detail}).",
  "error.version_mismatch":
    "Cette version de CallOut n'est pas compatible avec le serveur. Mettez à jour CallOut ou le serveur.",
  "error.room_not_found": "Aucun salon ne correspond à ce code.",
  "error.room_full": "Le salon est plein.",
  "error.already_in_room": "Quittez d'abord votre salon actuel.",
  "error.not_in_room": "Vous n'êtes dans aucun salon.",
  "error.rate_limited": "Trop de requêtes ; patientez un instant et réessayez.",
  "error.server_error": "Erreur du serveur ({detail}).",
  "error.not_owner": "Seule la personne qui a créé le salon peut le fermer.",
  "error.room_closed": "Le salon a été fermé par son créateur.",
};
