"""Catálogo español -> alemán. La clave es el texto en español tal como aparece en tr()/tr_n()."""

MESSAGES: dict[str, str] = {
    # --- árbol de juegos
    "Mods": "Mods",
    "Mapas": "Karten",
    "(base)": "(Basis)",
    "otros…": "weitere…",
    # --- tabla de servidores
    "País": "Land",
    "Servidor": "Server",
    "Ping": "Ping",
    "Jugadores": "Spieler",
    "Mapa": "Karte",
    "Modo": "Modus",
    "Versión": "Version",
    "País por IP: IP to Country Lite by DB-IP (db-ip.com), CC BY 4.0":
        "Land nach IP: IP to Country Lite by DB-IP (db-ip.com), CC BY 4.0",
    "{name}: el favorito no respondió": "{name}: Der Favorit hat nicht geantwortet",
    " — sin respuesta": " — keine Antwort",
    # --- menús, acciones y barra de herramientas
    "Archivo": "Datei",
    "Ver": "Ansicht",
    "Filtro": "Filter",
    "Ayuda": "Hilfe",
    "Herramientas": "Symbolleiste",
    "Refrescar lista": "Liste aktualisieren",
    "Detener": "Stopp",
    "Refrescar este servidor": "Diesen Server aktualisieren",
    "Conectar": "Verbinden",
    "Agregar a favoritos": "Zu Favoriten hinzufügen",
    "Quitar de favoritos": "Aus Favoriten entfernen",
    "Editar nota…": "Notiz bearbeiten…",
    "Nota para {address}": "Notiz für {address}",
    "Copiar IP:puerto": "IP:Port kopieren",
    "Ajustes": "Einstellungen",
    "Automático": "Automatisch",
    "Idioma": "Sprache",
    "El idioma cambiará al reiniciar JASE.":
        "Die Sprache wird nach einem Neustart von JASE geändert.",
    "Entendido": "Verstanden",
    "Tema": "Design",
    "Buscar juegos…": "Spiele suchen…",
    "Buscar jugador…": "Spieler suchen…",
    "Nombre del jugador": "Spielername",
    "Jugador": "Spieler",
    "Busca en el juego cargado en esta sesión": "Sucht im in dieser Sitzung geladenen Spiel",
    "Busca en los {n} juegos cargados en esta sesión":
        "Sucht in den {n} in dieser Sitzung geladenen Spielen",
    "Quitar filtros de mod/mapa": "Mod-/Kartenfilter entfernen",
    "El servidor está oculto por los filtros activos":
        "Der Server wird durch die aktiven Filter ausgeblendet",
    "Salir": "Beenden",
    "Acerca de": "Über",
    "Acerca de {name}": "Über {name}",
    "Versión {version}": "Version {version}",
    "Sistema": "System",
    "Oscuro": "Dunkel",
    "Claro": "Hell",
    "Neón": "Neon",
    "Marrón": "Braun",
    "Verde": "Grün",
    "Clásico": "Klassisch",
    "Windows XP": "Windows XP",
    "Synthwave": "Synthwave",
    "Ámbar": "Bernstein",
    "Terminal": "Terminal",
    "Blanco": "Weiß",
    "Sepia": "Sepia",
    "Color del fósforo": "Phosphorfarbe",
    "Color del fósforo:": "Phosphorfarbe:",
    "Óxido": "Rost",
    "GoldSrc": "GoldSrc",
    "Tema: {name}": "Design: {name}",
    "Apariencia": "Darstellung",
    "Tema de la interfaz:": "Design der Oberfläche:",
    # --- barra lateral y filtros
    "Filtrar servidores…": "Server filtern…",
    "Versión del juego": "Spielversion",
    "No aplica en Favoritos": "Gilt nicht für Favoriten",
    "★ Favoritos": "★ Favoriten",
    "Favoritos del juego seleccionado: se consultan directamente, sin master":
        "Favoriten des gewählten Spiels: werden direkt abgefragt, ohne Master-Server",
    "Ocultar vacíos": "Leere ausblenden",
    "Ocultar llenos": "Volle ausblenden",
    "Ocultar con contraseña": "Mit Passwort ausblenden",
    "Ocultar servidores con ping mayor": "Server mit höherem Ping ausblenden",
    "Ping máx.": "Max. Ping",
    "Sin límite": "Kein Limit",
    # --- detalle
    "Nombre": "Name",
    "Puntos": "Punkte",
    "Variable": "Variable",
    "Valor": "Wert",
    # --- barra de estado
    "{n} servidor": "{n} Server",
    "{n} servidores": "{n} Server",
    " · filtro de mod/mapa activo": " · Mod-/Kartenfilter aktiv",
    " · filtro de mod/mapa (no aplica en Favoritos)":
        " · Mod-/Kartenfilter (gilt nicht für Favoriten)",
    "Consultando %v / %m": "Abfrage %v / %m",
    "Escaneando…": "Scanne…",
    "Consultando masters de {name}…": "Frage Master-Server von {name} ab…",
    "{name}: lista guardada de la sesión": "{name}: in dieser Sitzung gespeicherte Liste",
    "{responded} de {listed} servidores respondieron":
        "{responded} von {listed} Servern haben geantwortet",
    "Consultando favoritos…": "Frage Favoriten ab…",
    "Consultando 1 favorito de {name}…": "Frage 1 Favoriten von {name} ab…",
    "Consultando {n} favoritos de {name}…": "Frage {n} Favoriten von {name} ab…",
    "No hay favoritos de {name}: agrégalos con clic derecho, con la estrella ☆ de la ficha o "
    "con Servidor → Agregar servidor…":
        "Noch keine Favoriten für {name}: Füge sie per Rechtsklick, mit dem Stern ☆ in der "
        "Serverkarte oder über Server → Server hinzufügen… hinzu",
    "Error al consultar favoritos: {error}": "Fehler beim Abfragen der Favoriten: {error}",
    "Favoritos: {online} de {total} respondieron":
        "Favoriten: {online} von {total} haben geantwortet",
    "Quitado de favoritos: {address}": "Aus Favoriten entfernt: {address}",
    "Agregado a favoritos: {address}": "Zu Favoriten hinzugefügt: {address}",
    # Agregar servidor a mano
    "Agregar servidor…": "Server hinzufügen…",
    "Agregar servidor": "Server hinzufügen",
    "Agregar favorito…": "Favorit hinzufügen…",
    "IP o nombre (se puede pegar IP:puerto)": "IP oder Hostname (IP:Port einfügen möglich)",
    "Puerto": "Port",
    "Juego:": "Spiel:",
    "IP:": "IP:",
    "Puerto:": "Port:",
    "{address} no respondió; quedó en favoritos":
        "{address} hat nicht geantwortet; in Favoriten gespeichert",
    "{address} no respondió; para guardarlo agrégalo como favorito":
        "{address} hat nicht geantwortet; füge ihn als Favorit hinzu, um ihn zu behalten",
    "{address} ya estaba en favoritos": "{address} war bereits in den Favoriten",
    "Servidor agregado: {address}": "Server hinzugefügt: {address}",
    "Detectar": "Erkennen",
    "Consulta la dirección con cada protocolo y elige el juego que responde":
        "Fragt die Adresse mit jedem Protokoll ab und wählt das Spiel, das antwortet",
    "Pega aquí varias direcciones IP:puerto (una por línea o mezcladas con texto)":
        "Füge hier mehrere IP:Port-Adressen ein (eine pro Zeile oder gemischt mit Text)",
    "Varias direcciones…": "Mehrere Adressen…",
    "Una dirección": "Eine Adresse",
    "Direcciones:": "Adressen:",
    "Detectando el juego de {address}…": "Erkenne das Spiel auf {address}…",
    "{address} no respondió como ningún juego conocido":
        "{address} hat als kein bekanntes Spiel geantwortet",
    "Detectado: {game}": "Erkannt: {game}",
    "Puede ser: {games}": "Möglicherweise: {games}",
    "Agregar 1 servidor": "1 Server hinzufügen",
    "Agregar {n} servidores": "{n} Server hinzufügen",
    "El portapapeles no tiene ninguna dirección IP:puerto":
        "Die Zwischenablage enthält keine IP:Port-Adresse",
    "Consultando 1 servidor…": "Frage 1 Server ab…",
    "Consultando {n} servidores…": "Frage {n} Server ab…",
    "1 favorito agregado ({missing} sin respuesta)":
        "1 Favorit hinzugefügt ({missing} ohne Antwort)",
    "{n} favoritos agregados ({missing} sin respuesta)":
        "{n} Favoriten hinzugefügt ({missing} ohne Antwort)",
    "1 servidor respondió de {total}": "1 von {total} Servern hat geantwortet",
    "{n} servidores respondieron de {total}": "{n} von {total} Servern haben geantwortet",
    "🕘 Recientes": "🕘 Zuletzt",
    "Servidores a los que te conectaste, del más reciente al más antiguo":
        "Server, mit denen du verbunden warst, neueste zuerst",
    "No aplica en Recientes": "Gilt nicht für Zuletzt",
    " · filtro de mod/mapa (no aplica en Recientes)":
        " · Mod-/Kartenfilter (gilt nicht für Zuletzt)",
    "Consultando recientes…": "Frage zuletzt besuchte Server ab…",
    "Consultando 1 servidor reciente de {name}…":
        "Frage 1 zuletzt besuchten Server von {name} ab…",
    "Consultando {n} servidores recientes de {name}…":
        "Frage {n} zuletzt besuchte Server von {name} ab…",
    "No hay servidores recientes de {name}": "Keine zuletzt besuchten Server von {name}",
    "Recientes: {online} de {total} respondieron":
        "Zuletzt: {online} von {total} haben geantwortet",
    "Quitar de recientes": "Aus Zuletzt entfernen",
    "Quitado de recientes: {address}": "Aus Zuletzt entfernt: {address}",
    "Dirección copiada: {address}": "Adresse kopiert: {address}",
    "Consultando {address}…": "Frage {address} ab…",
    "Servidor actualizado: {address}": "Server aktualisiert: {address}",
    "Avisar cuando haya sitio": "Benachrichtigen, wenn ein Platz frei ist",
    "Dejar de vigilar": "Nicht mehr beobachten",
    "Vigilando {name}…": "Beobachte {name}…",
    "Hay sitio": "Platz frei",
    "Minimizar a la bandeja del sistema": "In den Infobereich minimieren",
    "Mostrar JASE": "JASE anzeigen",
    "Hay sitio en {name} ({players}/{max}). ¿Conectar ahora?":
        "Auf {name} ist ein Platz frei ({players}/{max}). Jetzt verbinden?",
    "Se dejó de vigilar {name}: {minutes} min sin sitio":
        "{name} wird nicht mehr beobachtet: {minutes} Min. ohne freien Platz",
    "Error al consultar: {error}": "Fehler bei der Abfrage: {error}",
    "Copiar enlace jase://": "jase://-Link kopieren",
    "Enlace jase://": "jase://-Link",
    "El enlace no es válido.": "Der Link ist ungültig.",
    "¿Conectar a {name}?\n\nJuego: {game}\nDirección: {address}":
        "Mit {name} verbinden?\n\nSpiel: {game}\nAdresse: {address}",
    "Enlace copiado": "Link kopiert",
    "El servidor no respondió": "Der Server hat nicht geantwortet",
    "El servidor no responde; refresca para conectar":
        "Der Server antwortet nicht; aktualisiere, um zu verbinden",
    "Lanzando {name} → {address}": "Starte {name} → {address}",
    "Contraseña olvidada": "Gespeichertes Passwort entfernt",
    "Descargando base de países…": "Lade Länderdatenbank herunter…",
    "Base de países actualizada": "Länderdatenbank aktualisiert",
    "Lista de masters actualizada ({date})": "Master-Liste aktualisiert ({date})",
    "Base de países no disponible": "Länderdatenbank nicht verfügbar",
    "No se pudo guardar la configuración: {error}":
        "Die Einstellungen konnten nicht gespeichert werden: {error}",
    # --- diálogos
    "Contraseña": "Passwort",
    "No se pudo guardar la contraseña: {error}\nSe usará solo para esta conexión.":
        "Das Passwort konnte nicht gespeichert werden: {error}\n"
        "Es wird nur für diese Verbindung verwendet.",
    "No se pudo conectar": "Verbindung fehlgeschlagen",
    "No se pudo abrir Steam. ¿Está instalado?":
        "Steam konnte nicht geöffnet werden. Ist es installiert?",
    "Se conecta mediante Steam; no necesita carpeta":
        "Verbindet über Steam; kein Ordner nötig",
    "Revisa la ruta del juego en Ajustes.": "Prüfe den Spielpfad in den Einstellungen.",
    "Ruta no configurada": "Pfad nicht festgelegt",
    "No hay ruta configurada para {name}.": "Für {name} ist kein Pfad festgelegt.",
    "Configúrala en Ajustes para poder conectarte.":
        "Lege ihn in den Einstellungen fest, um dich verbinden zu können.",
    "Abrir ajustes": "Einstellungen öffnen",
    "Cancelar": "Abbrechen",
    "Buscar juegos": "Spiele suchen",
    "No hay juegos configurados. ¿Buscar juegos instalados?":
        "Keine Spiele eingerichtet. Nach installierten Spielen suchen?",
    "Ahora no": "Nicht jetzt",
    # --- contraseña, ajustes y buscar juegos
    "Contraseña del servidor": "Serverpasswort",
    "El servidor «{name}» requiere contraseña:": "Der Server „{name}“ erfordert ein Passwort:",
    "Olvidar contraseña": "Passwort vergessen",
    "Carpeta de instalación": "Installationsordner",
    "Examinar…": "Durchsuchen…",
    "Descargar imágenes de mapas desde internet (GameTracker)":
        "Kartenbilder aus dem Internet laden (GameTracker)",
    "Si el mapa no está instalado, busca su imagen en image.gametracker.com":
        "Wenn die Karte nicht installiert ist, wird ihr Bild auf image.gametracker.com gesucht",
    "Actualizar base de países": "Länderdatenbank aktualisieren",
    "Descarga de nuevo la base «IP to Country Lite» de DB-IP (db-ip.com)":
        "Lädt die Datenbank „IP to Country Lite“ von DB-IP (db-ip.com) erneut herunter",
    "No se pudo descargar la base de países":
        "Die Länderdatenbank konnte nicht heruntergeladen werden",
    "No se pudo leer la base de países": "Die Länderdatenbank konnte nicht gelesen werden",
    "No se pudo cargar la base de países": "Die Länderdatenbank konnte nicht geladen werden",
    "Error en el escaneo: {error}": "Fehler beim Scannen: {error}",
    "Guardar": "Speichern",
    "Rutas de instalación de los juegos:": "Installationspfade der Spiele:",
    "Descargando en segundo plano…": "Wird im Hintergrund heruntergeladen…",
    "{exe} con la carpeta {folder}": "{exe} mit dem Ordner {folder}",
    "No se encontró {wanted}": "{wanted} nicht gefunden",
    "Carpeta de {name}": "Ordner von {name}",
    "Cancelar búsqueda": "Suche abbrechen",
    "Buscar en carpeta…": "In Ordner suchen…",
    "Aplicar": "Übernehmen",
    "Buscando…": "Suche…",
    "Búsqueda cancelada": "Suche abgebrochen",
    "Buscar juegos en la carpeta": "Spiele im Ordner suchen",
    "1 juego encontrado": "1 Spiel gefunden",
    "{n} juegos encontrados": "{n} Spiele gefunden",
    "No se encontraron juegos": "Keine Spiele gefunden",
    "ya configurado": "bereits eingerichtet",
    "ruta guardada no encontrada": "gespeicherter Pfad nicht gefunden",
    "nuevo": "neu",
    "registro": "Registry",
    "steam": "Steam",
    "carpeta típica": "üblicher Ordner",
    "carpeta elegida": "gewählter Ordner",
    # --- ficha del servidor
    "Selecciona un servidor": "Wähle einen Server",
    "🔒 Contraseña": "🔒 Passwort",
    "El servidor requiere contraseña": "Der Server erfordert ein Passwort",
    "Copiar": "Kopieren",
    "Copiar IP:puerto al portapapeles": "IP:Port in die Zwischenablage kopieren",
    "Sin respuesta": "Keine Antwort",
    "Sin historial todavía": "Noch kein Verlauf",
    "Jugadores promedio por hora": "Durchschnittliche Spieler pro Stunde",
    # --- núcleo (scanner y launcher)
    "Master {host} no responde": "Master {host} antwortet nicht",
    "La clave de API de Steam no es válida; revísala en Ajustes → General":
        "Der Steam-API-Schlüssel ist ungültig; prüfe ihn unter Einstellungen → Allgemein",
    "Steam limitó las consultas; prueba en unos minutos":
        "Steam hat die Anfragen begrenzt; versuche es in ein paar Minuten erneut",
    "Falta la clave de API de Steam; ponla en Ajustes → General":
        "Der Steam-API-Schlüssel fehlt; trage ihn unter Einstellungen → Allgemein ein",
    "Solo se admite {host} como fuente de Steam":
        "Nur {host} wird als Steam-Quelle unterstützt",
    "Ningún master respondió": "Kein Master hat geantwortet",
    "Clave de API de Steam": "Steam-Web-API-Schlüssel",
    "Incluir servidores vacíos de Steam (lista mucho más larga)":
        "Leere Steam-Server einbeziehen (viel längere Liste)",
    "Consíguela en {link} (cualquier dominio sirve)":
        "Erhältlich unter {link} (jede Domain funktioniert)",
    "{game}: falta la clave de API de Steam; ponla en Ajustes → General":
        "{game}: Der Steam-Web-API-Schlüssel fehlt; trage ihn unter Einstellungen → Allgemein ein",
    "No se pudo guardar la clave de Steam: {error}\nSe usará solo en esta sesión.":
        "Der Steam-Schlüssel konnte nicht gespeichert werden: {error}\n"
        "Er wird nur in dieser Sitzung verwendet.",
    "{n} servidor en el master, consultando…": "{n} Server auf dem Master, frage ab…",
    "{n} servidores en el master, consultando…": "{n} Server auf dem Master, frage ab…",
    "No hay ruta configurada para {name}. Configúrala en Ajustes.":
        "Für {name} ist kein Pfad festgelegt. Lege ihn in den Einstellungen fest.",
    "No se encontró {exe} en {folder}. Revisa la ruta en Ajustes.":
        "{exe} wurde in {folder} nicht gefunden. Prüfe den Pfad in den Einstellungen.",
    "Juegos": "Spiele",
    "Master": "Master",
    "Masters": "Masters",
    "General": "Allgemein",
    "Activo": "Aktiv",
    "Origen": "Quelle",
    "Prueba": "Test",
    "Oficial": "Offiziell",
    "Personal": "Eigene",
    "Agregar…": "Hinzufügen…",
    "Quitar": "Entfernen",
    "Probar": "Testen",
    "Probando…": "Teste…",
    "Restaurar oficiales": "Offizielle wiederherstellen",
    "Agregar master": "Master hinzufügen",
    "Dirección del master (host:puerto):": "Master-Adresse (Host:Port):",
    "Dirección no válida; usa host:puerto (puerto 1-65535).":
        "Ungültige Adresse; verwende Host:Port (Port 1-65535).",
    "Ese master ya está en la lista.": "Dieser Master ist bereits in der Liste.",
    "✓ 1 servidor": "✓ 1 Server",
    "✓ {n} servidores": "✓ {n} Server",
    "✗ no responde": "✗ keine Antwort",
    'Los masters personales se quitan con "Quitar"':
        "Eigene Master werden mit „Entfernen“ gelöscht",
    "Lista oficial: {date}": "Offizielle Liste: {date}",
    "Buscar actualización": "Nach Updates suchen",
    "Buscando actualización…": "Suche nach Updates…",
    "La lista de masters está al día": "Die Master-Liste ist aktuell",
    "No se pudo descargar la lista de masters":
        "Die Master-Liste konnte nicht heruntergeladen werden",
    "{game}: no hay masters activos; revísalos en Ajustes → Masters":
        "{game}: keine aktiven Master; prüfe sie unter Einstellungen → Masters",
    "Desactivado": "Aus",
    "Cada {n} min": "Alle {n} Min.",
    "Refrescar automáticamente": "Automatisch aktualisieren",
    "Avisar si hay una versión nueva de JASE":
        "Benachrichtigen, wenn eine neue JASE-Version verfügbar ist",
    "Nueva versión {version} disponible": "Neue Version {version} verfügbar",
    "Exportar favoritos…": "Favoriten exportieren…",
    "Importar favoritos…": "Favoriten importieren…",
    "Archivos JSON (*.json)": "JSON-Dateien (*.json)",
    "Favoritos exportados a {path}": "Favoriten exportiert nach {path}",
    "{n} favorito importado, {skipped} descartados":
        "{n} Favorit importiert, {skipped} übersprungen",
    "{n} favoritos importados, {skipped} descartados":
        "{n} Favoriten importiert, {skipped} übersprungen",
    "No se pudo leer el archivo: {error}": "Die Datei konnte nicht gelesen werden: {error}",
    "El archivo es demasiado grande para ser una exportación de favoritos":
        "Die Datei ist zu groß für einen Favoriten-Export",
    "El archivo no es una exportación válida de favoritos de JASE":
        "Die Datei ist kein gültiger JASE-Favoriten-Export",
    "Filtros guardados": "Gespeicherte Filter",
    "Guardar filtros actuales…": "Aktuelle Filter speichern…",
    "Nombre de la combinación:": "Name für diese Kombination:",
    "Borrar": "Löschen",
    "Amigo en línea": "Freund online",
    "{friend} está en {server} ({game})": "{friend} ist auf {server} ({game})",
    "{n} amigo está en línea: {friend}": "{n} Freund ist online: {friend}",
    "{n} amigos están en línea: {friend}": "{n} Freunde sind online: {friend}",
    "Amigos…": "Freunde…",
    "Amigos": "Freunde",
    "Revisar favoritos y recientes cada 2 min, también con la ventana en la bandeja":
        "Favoriten und zuletzt besuchte Server alle 2 Min. prüfen, auch wenn das Fenster im "
        "Infobereich ist",
    "Agregar": "Hinzufügen",
    "Agregar a amigos": "Zu Freunden hinzufügen",
    "Hay una versión nueva de JASE ({version}).": "Es gibt eine neue Version von JASE ({version}).",
    "Descargar e instalar": "Herunterladen und installieren",
    "Ver la página": "Seite anzeigen",
    "Descargando {name}…": "Lade {name} herunter…",
    "Actualizar JASE": "JASE aktualisieren",
    "No se pudo ejecutar el instalador.":
        "Das Installationsprogramm konnte nicht gestartet werden.",
    "el archivo descargado está incompleto": "die heruntergeladene Datei ist unvollständig",
    "el archivo es más grande de lo anunciado": "die Datei ist größer als angegeben",
    "la suma SHA-256 no coincide": "die SHA-256-Prüfsumme stimmt nicht überein",
    "la descarga no usa una conexión segura (https)":
        "der Download verwendet keine sichere Verbindung (https)",
    "descarga cancelada": "Download abgebrochen",
    "error de red o de disco": "Netzwerk- oder Datenträgerfehler",
    "No se pudo descargar la actualización: {error}":
        "Das Update konnte nicht heruntergeladen werden: {error}",
    "Cerrar": "Schließen",
    "{n} jugador": "{n} Spieler",
    "{n} jugadores": "{n} Spieler",
    "{n} bot": "{n} Bot",
    "{n} bots": "{n} Bots",
    "Ocultar solo bots": "Nur-Bot-Server ausblenden",
    "Oculta los servidores cuyos jugadores son todos bots":
        "Blendet Server aus, auf denen alle Spieler Bots sind",
    "Mostrar solo servidores de un país": "Nur Server aus einem Land anzeigen",
    "Todos los países": "Alle Länder",
    "Copiar favoritos como texto": "Favoriten als Text kopieren",
    "No hay favoritos de {name} para copiar": "Keine {name}-Favoriten zum Kopieren",
    "{n} favorito copiado como texto": "{n} Favorit als Text kopiert",
    "{n} favoritos copiados como texto": "{n} Favoriten als Text kopiert",
    "Avisar cuando se arme partida": "Benachrichtigen, wenn eine Runde zustande kommt",
    "Avisar cuando se arme partida…": "Benachrichtigen, wenn eine Runde zustande kommt…",
    "Avisar cuando {server} tenga al menos estos jugadores (0 = no avisar):":
        "Benachrichtigen, wenn {server} mindestens so viele Spieler hat (0 = aus):",
    "Avisaré cuando {server} tenga {n} jugador": "Ich melde mich, wenn {server} {n} Spieler hat",
    "Avisaré cuando {server} tenga {n} jugadores": "Ich melde mich, wenn {server} {n} Spieler hat",
    "Ya no se avisa de partidas en {server}": "Keine Rundenbenachrichtigungen mehr für {server}",
    "Se armó partida": "Eine Runde läuft",
    "{server}: {n} jugador en {game}": "{server}: {n} Spieler in {game}",
    "{server}: {n} jugadores en {game}": "{server}: {n} Spieler in {game}",
    "Avisar de partida: {n} jugador…": "Rundenalarm: {n} Spieler…",
    "Avisar de partida: {n} jugadores…": "Rundenalarm: {n} Spieler…",
    "Grupo de favoritos que se muestra": "Angezeigte Favoritengruppe",
    "Grupo": "Gruppe",
    "Todos los grupos": "Alle Gruppen",
    "Sin grupo": "Ohne Gruppe",
    "Nuevo grupo…": "Neue Gruppe…",
    "Nombre del grupo:": "Gruppenname:",
    "{server} pasó al grupo «{group}»": "{server} ist jetzt in der Gruppe „{group}“",
    "{server} quedó sin grupo": "{server} hat jetzt keine Gruppe",
    "Servidores con gente en tus juegos instalados, de más a menos jugadores (sin contar bots). "
    "Doble clic para verlo.":
        "Server mit Spielern in deinen installierten Spielen, die vollsten zuerst (ohne Bots). "
        "Doppelklick zeigt ihn an.",
    "No hay juegos listos: configura sus carpetas en Ajustes o con Archivo → Buscar juegos…":
        "Keine Spiele bereit: Lege ihre Ordner in den Einstellungen fest oder mit Datei → Spiele "
        "suchen…",
    "Ahora se juega": "Jetzt wird gespielt",
    "Ahora se juega…": "Jetzt wird gespielt…",
    "Juego": "Spiel",
    "Actualizar": "Aktualisieren",
    "{n} jugador en tus juegos": "{n} Spieler in deinen Spielen",
    "{n} jugadores en tus juegos": "{n} Spieler in deinen Spielen",
    "escaneando 1 juego…": "1 Spiel wird durchsucht…",
    "escaneando {n} juegos…": "{n} Spiele werden durchsucht…",
    "Buscar en la red local": "Im lokalen Netzwerk suchen",
    "Buscando servidores de {name} en la red local…": "Suche {name}-Server im lokalen Netzwerk…",
    "{n} servidor en la red local, consultando…": "{n} Server im lokalen Netzwerk, wird abgefragt…",
    "{n} servidores en la red local, consultando…":
        "{n} Server im lokalen Netzwerk, werden abgefragt…",
    "El master {host} pide esperar unos segundos entre consultas":
        "Der Master {host} verlangt einige Sekunden Pause zwischen Abfragen",
    "Buscar actualizaciones…": "Nach Updates suchen…",
    "Buscar actualizaciones": "Nach Updates suchen",
    "Buscando actualizaciones…": "Suche nach Updates…",
    "Tienes la última versión ({version}).": "Du hast die neueste Version ({version}).",
    "No se pudo consultar GitHub. Revisa la conexión e inténtalo más tarde.":
        "GitHub war nicht erreichbar. Prüfe die Verbindung und versuche es später erneut.",
    "Consola RCON…": "RCON-Konsole…",
    "Consola RCON: {server}": "RCON-Konsole: {server}",
    "El servidor no respondió al RCON.": "Der Server hat auf RCON nicht geantwortet.",
    "Clave RCON incorrecta.": "Falsches RCON-Passwort.",
    "El servidor no dio un challenge de RCON (¿tiene RCON activado?).":
        "Der Server hat keine RCON-Challenge geschickt (ist RCON aktiviert?).",
    "No se pudo conectar al RCON (TCP) del servidor.":
        "Verbindung zum RCON (TCP) des Servers fehlgeschlagen.",
    "Respuesta RCON inválida.": "Ungültige RCON-Antwort.",
    "Se cortó la conexión RCON.": "Die RCON-Verbindung wurde getrennt.",
    "Este juego no tiene RCON en JASE.": "Dieses Spiel hat in JASE kein RCON.",
    "Error de red: {error}": "Netzwerkfehler: {error}",
    "Recordar la clave": "Passwort merken",
    "Se guarda en el Administrador de credenciales":
        "Wird in der Anmeldeinformationsverwaltung gespeichert",
    "Comando (p. ej. status, map mp_toujane)": "Befehl (z. B. status, map mp_toujane)",
    "Enviar": "Senden",
    "Servidor:": "Server:",
    "Clave RCON:": "RCON-Passwort:",
    "(sin respuesta de texto)": "(keine Textantwort)",
    "el zip descargado no es una versión portátil válida de JASE":
        "die heruntergeladene ZIP ist keine gültige portable JASE-Version",
    "No se pudo reemplazar la carpeta de JASE; actualízala a mano.":
        "Der JASE-Ordner konnte nicht ersetzt werden; aktualisiere ihn von Hand.",
    "No se encontró «{command}» para abrir el juego. Instala Wine o cambia el comando en Ajustes "
    "→ General.":
        "„{command}“ zum Starten des Spiels nicht gefunden. Installiere Wine oder ändere den "
        "Befehl unter Einstellungen → Allgemein.",
    "vacío = abrir el .exe directamente": "leer = die .exe direkt starten",
    "Programa con el que se abren los juegos de Windows en Linux (p. ej. wine, umu-run o un "
    "script propio)":
        "Programm, mit dem Windows-Spiele unter Linux gestartet werden (z. B. wine, umu-run oder "
        "ein eigenes Skript)",
    "Abrir juegos con (Wine)": "Spiele öffnen mit (Wine)",
}
