import { browser } from "$app/environment";

export type Lang = "en" | "ru";
const KEY = "aegisvpn.lang";

type Dict = Record<string, string>;

const EN: Dict = {
  // nav
  "nav.home": "Home",
  "nav.split": "Split",
  "nav.settings": "Settings",

  // home / connection
  "status.disconnected": "Not connected",
  "status.connecting": "Connecting",
  "status.connected": "Connected",
  "conn.selectLocation": "Select a location first",
  "home.empty": "No subscriptions yet. Tap + in the top-right corner.",
  "home.autoUpdate": "auto-update {h}h",
  "home.expires": "Expires: {date}",

  // subscription menu
  "menu.info": "Subscription info",
  "menu.rename": "Rename",
  "menu.remove": "Remove subscription",

  // info modal
  "info.url": "URL",
  "info.imported": "Imported",
  "info.autoUpdate": "Auto-update",
  "info.everyH": "every {h} h",
  "info.traffic": "Traffic",
  "info.expires": "Expires",
  "info.servers": "Servers",
  "info.support": "Support",

  // rename modal
  "rename.title": "Rename subscription",

  // import modal
  "import.title": "Add subscription",
  "import.hint": "Paste a subscription URL or a single vless:// link.",
  "import.importing": "Importing…",
  "import.add": "Add",

  // generic
  "common.close": "Close",
  "common.cancel": "Cancel",
  "common.save": "Save",

  // split
  "split.title": "Split tunneling",
  "split.apps": "Apps",
  "split.websites": "Websites",
  "split.mode": "Mode",
  "split.modeGeneral": "General",
  "split.modeSelective": "Selective",
  "split.active": "{n} active",
  "split.mode.appsSelective": "VPN works only for the selected apps. Everything else stays direct.",
  "split.mode.appsGeneral": "VPN works for all apps except those selected (which stay direct).",
  "split.mode.sitesSelective": "VPN works only on the selected websites. Everything else stays direct.",
  "split.mode.sitesGeneral": "VPN works on all websites except those selected (which stay direct).",
  "split.searchApps": "Search apps",
  "split.noAppsTitle": "No apps yet",
  "split.noAppsHint": "Tap + to pick from your installed apps, or choose one by file.",
  "split.noAppsMatch": "No apps match the query.",
  "split.sitePlaceholder": "example.com or *.example.com",
  "split.noSitesTitle": "No websites yet",
  "split.noSitesHint": "Add a hostname (example.com) or a wildcard pattern (*.example.com).",
  "split.addApp": "Add app",
  "split.srcInstalled": "Installed",
  "split.srcRunning": "Running",
  "split.searchInstalled": "Search installed apps",
  "split.loadingApps": "Loading installed apps…",
  "split.noInstalled": "No installed apps found.",
  "split.noInstalledMatch": "Nothing matches your search.",
  "split.pickFileHint": "Don't see your app (e.g. a Steam game)? Pick its .desktop file or executable.",
  "split.manualHint": "Don't see your app (e.g. a Steam game)? Type its process name, or choose its file.",
  "split.manualPlaceholder": "Process name (e.g. cs2)",
  "split.chooseFile": "Choose from file…",

  // settings
  "settings.title": "Settings",
  "settings.appearance": "Appearance",
  "settings.dark": "Dark",
  "settings.light": "Light",
  "settings.general": "General",
  "settings.language": "Language",
  "settings.killswitch": "Killswitch",
  "settings.killswitchSub": "Block all traffic if the VPN connection drops.",
  "settings.allowLan": "Allow LAN traffic",
  "settings.allowLanSub": "Keep printers, NAS, and local devices reachable.",

  // VPN mode
  "settings.vpnMode": "VPN mode",
  "mode.tun": "TUN (system-wide)",
  "mode.proxy": "Proxy (SOCKS/HTTP)",
  "mode.tunSub": "Routes every app. Needs the system helper.",
  "mode.proxySub": "Local proxy at 127.0.0.1:2080 — no root. Point your apps/system at it.",

  // VPN core (sing-box)
  "settings.core": "VPN core",
  "core.checking": "Checking for updates…",
  "core.checkFailed": "Couldn't check for updates",
  "core.notInstalled": "Not installed",
  "core.upToDate": "Up to date",
  "core.updateAvailable": "Update available",
  "core.latest": "latest {v}",
  "core.install": "Install",
  "core.update": "Update",
  "core.updating": "Downloading…",
  "core.versions": "Versions",
  "core.versionsTitle": "Pick a sing-box version",
  "core.preview": "pre-release",
  "core.currentlyInstalled": "currently installed",

  // Privileged helper
  "settings.helper": "System helper",
  "helper.title": "Helper service",
  "helper.ready": "Installed and running",
  "helper.notInstalled": "Not installed — required to connect",
  "helper.install": "Set up",
  "helper.reinstall": "Reinstall",
  "helper.installing": "Installing…",
};

const RU: Dict = {
  "nav.home": "Главная",
  "nav.split": "Сплит",
  "nav.settings": "Настройки",

  "status.disconnected": "Не подключено",
  "status.connecting": "Подключение",
  "status.connected": "Подключено",
  "conn.selectLocation": "Сначала выберите локацию",
  "home.empty": "Пока нет подписок. Нажмите + в правом верхнем углу.",
  "home.autoUpdate": "автообновление {h}ч",
  "home.expires": "Истекает: {date}",

  "menu.info": "Информация о подписке",
  "menu.rename": "Переименовать",
  "menu.remove": "Удалить подписку",

  "info.url": "Ссылка",
  "info.imported": "Добавлена",
  "info.autoUpdate": "Автообновление",
  "info.everyH": "каждые {h} ч",
  "info.traffic": "Трафик",
  "info.expires": "Истекает",
  "info.servers": "Серверы",
  "info.support": "Поддержка",

  "rename.title": "Переименовать подписку",

  "import.title": "Добавить подписку",
  "import.hint": "Вставьте ссылку на подписку или одиночную vless:// ссылку.",
  "import.importing": "Добавление…",
  "import.add": "Добавить",

  "common.close": "Закрыть",
  "common.cancel": "Отмена",
  "common.save": "Сохранить",

  "split.title": "Раздельный туннель",
  "split.apps": "Приложения",
  "split.websites": "Сайты",
  "split.mode": "Режим",
  "split.modeGeneral": "Общий",
  "split.modeSelective": "Выборочный",
  "split.active": "активно: {n}",
  "split.mode.appsSelective": "VPN работает только для выбранных приложений. Остальное — напрямую.",
  "split.mode.appsGeneral": "VPN работает для всех приложений, кроме выбранных (они идут напрямую).",
  "split.mode.sitesSelective": "VPN работает только на выбранных сайтах. Остальное — напрямую.",
  "split.mode.sitesGeneral": "VPN работает на всех сайтах, кроме выбранных (они идут напрямую).",
  "split.searchApps": "Поиск приложений",
  "split.noAppsTitle": "Пока нет приложений",
  "split.noAppsHint": "Нажмите +, чтобы выбрать из установленных приложений или указать файл.",
  "split.noAppsMatch": "Ничего не найдено по запросу.",
  "split.sitePlaceholder": "example.com или *.example.com",
  "split.noSitesTitle": "Пока нет сайтов",
  "split.noSitesHint": "Добавьте домен (example.com) или шаблон (*.example.com).",
  "split.addApp": "Добавить приложение",
  "split.srcInstalled": "Установленные",
  "split.srcRunning": "Запущенные",
  "split.searchInstalled": "Поиск установленных приложений",
  "split.loadingApps": "Загрузка приложений…",
  "split.noInstalled": "Установленные приложения не найдены.",
  "split.noInstalledMatch": "Ничего не найдено.",
  "split.pickFileHint": "Нет вашего приложения (например, игры Steam)? Выберите его .desktop-файл или исполняемый файл.",
  "split.manualHint": "Нет приложения в списке (например, игра Steam)? Впишите имя его процесса или выберите файл.",
  "split.manualPlaceholder": "Имя процесса (например cs2)",
  "split.chooseFile": "Выбрать файл…",

  "settings.title": "Настройки",
  "settings.appearance": "Оформление",
  "settings.dark": "Тёмная",
  "settings.light": "Светлая",
  "settings.general": "Общие",
  "settings.language": "Язык",
  "settings.killswitch": "Killswitch",
  "settings.killswitchSub": "Блокировать весь трафик, если VPN отключился.",
  "settings.allowLan": "Разрешить локальную сеть",
  "settings.allowLanSub": "Оставить доступными принтеры, NAS и локальные устройства.",

  "settings.vpnMode": "Режим VPN",
  "mode.tun": "TUN (всё устройство)",
  "mode.proxy": "Прокси (SOCKS/HTTP)",
  "mode.tunSub": "Весь трафик системы. Нужен системный хелпер.",
  "mode.proxySub": "Локальный прокси 127.0.0.1:2080 — без root. Укажи его в приложениях/системе.",

  "settings.core": "Ядро VPN",
  "core.checking": "Проверка обновлений…",
  "core.checkFailed": "Не удалось проверить обновления",
  "core.notInstalled": "Не установлено",
  "core.upToDate": "Актуальная версия",
  "core.updateAvailable": "Доступно обновление",
  "core.latest": "последняя {v}",
  "core.install": "Установить",
  "core.update": "Обновить",
  "core.updating": "Загрузка…",
  "core.versions": "Версии",
  "core.versionsTitle": "Выбор версии sing-box",
  "core.preview": "пре-релиз",
  "core.currentlyInstalled": "сейчас установлена",

  "settings.helper": "Системный хелпер",
  "helper.title": "Служба-хелпер",
  "helper.ready": "Установлен и работает",
  "helper.notInstalled": "Не установлен — нужен для подключения",
  "helper.install": "Установить",
  "helper.reinstall": "Переустановить",
  "helper.installing": "Установка…",
};

const DICTS: Record<Lang, Dict> = { en: EN, ru: RU };

function detect(): Lang {
  if (!browser) return "en";
  const stored = localStorage.getItem(KEY);
  if (stored === "ru" || stored === "en") return stored;
  return navigator.language?.toLowerCase().startsWith("ru") ? "ru" : "en";
}

class I18n {
  lang = $state<Lang>(detect());

  set(l: Lang): void {
    this.lang = l;
    if (browser) localStorage.setItem(KEY, l);
  }

  /** Translate a key, substituting {placeholders} from vars. Falls back to
   *  English, then to the key itself. */
  t(key: string, vars?: Record<string, string | number>): string {
    let s = DICTS[this.lang][key] ?? EN[key] ?? key;
    if (vars) {
      for (const [k, v] of Object.entries(vars)) {
        s = s.replaceAll(`{${k}}`, String(v));
      }
    }
    return s;
  }
}

export const i18n = new I18n();

/** Reactive translate helper — reads i18n.lang, so templates update on change. */
export function t(key: string, vars?: Record<string, string | number>): string {
  return i18n.t(key, vars);
}

export const LANGUAGES: { value: Lang; label: string }[] = [
  { value: "en", label: "English" },
  { value: "ru", label: "Русский" },
];
