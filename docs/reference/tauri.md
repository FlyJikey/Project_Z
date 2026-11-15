# Справка: Tauri 2 и фронтенд-стек

Дата сверки: 2026-11-15. Все версии получены из реестров (`npm view`, `crates.io/api`) в этот день. Каждое утверждение снабжено ссылкой на источник; то, что подтвердить не удалось или что выведено из исходников, вынесено в раздел «Не подтверждено» в конце.

Сокращения ссылок: **TD** — https://v2.tauri.app, **DRS** — https://docs.rs, **SRC** — исходники на GitHub (указан тег/ветка).

---

## 1. Версии (стабильные, ноябрь 2026)

| Пакет | Версия | Источник |
|---|---|---|
| `tauri` (crate) | **2.12.5** (2026-11-06) | https://crates.io/crates/tauri |
| `tauri-build` (crate) | **2.7.4** | https://crates.io/crates/tauri-build |
| `tauri-cli` (crate) | 2.12.4 | https://crates.io/crates/tauri-cli |
| `@tauri-apps/cli` | **2.12.4** | npm |
| `@tauri-apps/api` | **2.12.4** | npm |
| `tauri-plugin-notification` / `@tauri-apps/plugin-notification` | 2.8.1 / 2.8.1 | crates.io / npm |
| `tauri-plugin-updater` / `@tauri-apps/plugin-updater` | 2.14.1 / 2.14.1 | crates.io / npm |
| `tauri-plugin-autostart` / `@tauri-apps/plugin-autostart` | **2.7.0** / 2.7.0 (на crates.io есть ещё `3.0.0-alpha.3` — не брать) | crates.io / npm |
| `tauri-plugin-shell` / `@tauri-apps/plugin-shell` | 2.6.0 / 2.6.0 | crates.io / npm |
| `tauri-plugin-single-instance` | 2.5.5; **npm-пакета нет** (E404), JS-API у плагина нет | crates.io; npm; [TD single-instance](https://v2.tauri.app/plugin/single-instance/) |
| `tauri-plugin-opener` / `@tauri-apps/plugin-opener` | 2.7.3 / 2.7.3 | crates.io / npm |
| `tauri-plugin-process` / `@tauri-apps/plugin-process` | 2.4.0 / 2.4.0 (нужен для `relaunch()` из JS) | crates.io / npm |
| React / react-dom | **19.3.1** | npm |
| `@types/react` / `@types/react-dom` | 19.3.0 / 19.3.0 | npm |
| Vite | **8.3.8** (Node `^20.19.0 \|\| >=22.12.0`) | npm; [vite.dev/guide](https://vite.dev/guide/) |
| `@vitejs/plugin-react` | **6.3.0** (peer: `vite ^8`) | npm |
| TypeScript | **7.0.2** (dist-tag `latest`; бинарь `tsc`; официальный шаблон Vite закрепляет `~7.0.2`) | npm; [create-vite template-react-ts](https://github.com/vitejs/vite/blob/main/packages/create-vite/template-react-ts/package.json) |
| Tailwind CSS | **4.3.5** | npm |
| `@tailwindcss/vite` | **4.3.5** (peer: vite `^5.2 \|\| ^6 \|\| ^7 \|\| ^8`) | npm |
| `create-vite` | 9.3.4 | npm |

**MSRV:** Rust **1.90**. `rust-version = "1.90"` у `tauri 2.12.5`, `tauri-build 2.7.4`, `tauri-plugin-updater`, `-notification`, `-single-instance` (crates.io API); страницы плагинов тоже пишут «Rust 1.90 or later» ([updater](https://v2.tauri.app/plugin/updater/), [notification](https://v2.tauri.app/plugin/notification/), [autostart](https://v2.tauri.app/plugin/autostart/)). Шаблон `tauri init` создаёт крейт с `edition = "2024"`, `rust-version = "1.90"` ([SRC шаблон](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/templates/app/src-tauri/Cargo.crate-manifest)).

**Трей встроен в `tauri`**, это не плагин: включается cargo-фичей `tray-icon` ([TD system-tray](https://v2.tauri.app/learn/system-tray/); фича есть в features у `tauri 2.12.5`, crates.io API).

**Подключение Tailwind к Vite:** пакеты `tailwindcss` + `@tailwindcss/vite`, плагин в `vite.config.ts`, в CSS `@import "tailwindcss";` — без `tailwind.config.js` и postcss ([tailwindcss.com/docs/installation/using-vite](https://tailwindcss.com/docs/installation/using-vite)).

---

## 2. Раскладка: Tauri-крейт в `crates/app`, фронтенд в `ui/`

### 2.1 Как CLI находит проект (по исходникам CLI 2.12.4)

Источник: [SRC helpers/app_paths.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/src/helpers/app_paths.rs).

- Каталог Rust-приложения: берётся `$TAURI_APP_PATH`, иначе текущий каталог. Сначала проверяется сам каталог и `<каталог>/src-tauri` на наличие `tauri.conf.json` / `tauri.conf.json5` / `Tauri.toml`. Если нет — рекурсивный поиск вглубь, по умолчанию **глубина 3** (`TAURI_CLI_CONFIG_DEPTH`).
- Каталог фронтенда: `$TAURI_FRONTEND_PATH`, иначе текущий каталог; ищется `package.json` (та же глубина).
- `beforeDevCommand` по умолчанию выполняется в каталоге фронтенда (`cwd` можно переопределить в объектной форме) — [SRC dev.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/src/dev.rs).
- CLI знает про Cargo workspace (`workspace_dir`, отслеживание зависимостей из workspace) — [SRC interface/rust.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/src/interface/rust.rs).

**Рекомендация (по этим исходникам):** не полагаться на глубину поиска (`crates/app/tauri.conf.json` — ровно глубина 3), а задавать пути явно:

```bash
# из корня репозитория
export TAURI_APP_PATH=crates/app
export TAURI_FRONTEND_PATH=ui
npm --prefix ui run tauri dev          # CLI установлен в ui/ как devDependency
npm --prefix ui run tauri build -- --target universal-apple-darwin
```

Либо запускать из `crates/app` (`TAURI_FRONTEND_PATH=../../ui`). Пути в `tauri.conf.json` (`frontendDist`, `externalBin`, `resources`, `icon`) задаются **относительно каталога с `tauri.conf.json`** (док по `externalBin`: «relative to the `src-tauri` directory» — [TD sidecar](https://v2.tauri.app/develop/sidecar/); в нашей раскладке это `crates/app`).

### 2.2 Дерево

```
Project_Z/
  Cargo.toml                 # [workspace] members = ["crates/*"]
  crates/
    app/
      Cargo.toml  build.rs  tauri.conf.json
      capabilities/default.json
      permissions/*.toml     # опционально, свои разрешения (см. §3)
      icons/                 # tauri icon
      binaries/              # sidecar: name-<triple>[.exe]
      src/main.rs  src/lib.rs
    ipc-proto/ config-gen/ platform/ helper/
  ui/
    package.json  vite.config.ts  tsconfig*.json  index.html  src/
```

Роли файлов `src-tauri` (в нашем случае `crates/app`): `tauri.conf.json`, `capabilities/` (папка по умолчанию), `icons/`, `build.rs` (вызывает `tauri_build::build()`), `src/lib.rs` (код + мобильная точка входа), `src/main.rs` (вызывает `app_lib::run()`, `app_lib` = `[lib] name`) — [TD project-structure](https://v2.tauri.app/start/project-structure/).

### 2.3 `tauri.conf.json` (образец для нашей раскладки)

Поля подтверждены по [TD config reference](https://v2.tauri.app/reference/config/) и [JSON-схеме](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-schema-generator/schemas/config.schema.json). `devUrl`-порт должен совпадать с портом Vite ([TD vite](https://v2.tauri.app/start/frontend/vite/)).

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Project Z",
  "version": "0.1.0",
  "identifier": "com.example.projectz",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../../ui/dist"
  },
  "app": {
    "windows": [
      { "label": "main", "title": "Project Z", "width": 960, "height": 640 }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg", "nsis"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"],
    "createUpdaterArtifacts": true,
    "externalBin": ["binaries/xray", "binaries/sing-box", "binaries/helper"],
    "resources": { "resources/**/*": "resources/" },
    "macOS": {
      "minimumSystemVersion": "13.0",
      "signingIdentity": "-",
      "hardenedRuntime": true
    },
    "windows": {
      "nsis": { "installMode": "perMachine", "installerHooks": "./windows/hooks.nsh", "languages": ["Russian"] }
    }
  },
  "plugins": {
    "updater": {
      "pubkey": "СОДЕРЖИМОЕ .pub (не путь!)",
      "endpoints": ["https://github.com/OWNER/REPO/releases/latest/download/latest.json"],
      "windows": { "installMode": "passive" }
    }
  }
}
```

Подтверждено:
- `identifier` — обратный домен, только буквы/цифры/дефис/точка; значение по умолчанию `com.tauri.dev` отвергается `tauri build`. `frontendDist` — путь к каталогу (встраивается рекурсивно, точка входа `index.html`), `devUrl` — URL dev-сервера. `beforeDevCommand` — строка или `{script, cwd, wait}`; `beforeBuildCommand` — строка или `{script, cwd}` ([config reference](https://v2.tauri.app/reference/config/)).
- `bundle.targets`: `all` / один тип / массив из `deb, rpm, appimage, nsis, msi, app, dmg` (там же).
- `bundle.macOS.minimumSystemVersion` по умолчанию `"10.13"`; в `tauri dev` игнорируется (там же). Для ТЗ (macOS 13+) задать `"13.0"`.
- `bundle.macOS.hardenedRuntime` по умолчанию `true`; `signingIdentity` `"-"` — ad-hoc ([TD sign/macos](https://v2.tauri.app/distribute/sign/macos/)).
- NSIS: `installMode` ∈ `currentUser` (по умолчанию) / `perMachine` / `both`; `installerHooks` — `.nsh` с макросами `NSIS_HOOK_PREINSTALL`, `NSIS_HOOK_POSTINSTALL`, `NSIS_HOOK_PREUNINSTALL`, `NSIS_HOOK_POSTUNINSTALL`; `languages` — список языков (по умолчанию `["English"]`) ([config reference](https://v2.tauri.app/reference/config/), [TD windows-installer](https://v2.tauri.app/distribute/windows-installer/)). `perMachine` и `both` требуют прав администратора.
- Платформенные файлы `tauri.macos.conf.json`, `tauri.windows.conf.json` и т.д. автоматически сливаются с основным; `--config` принимает JSON/путь, сливаются по порядку ([TD cli reference](https://v2.tauri.app/reference/cli/)).
- `bundle.resources` — список или карта «источник → цель» с глобами; в карте при глобах структура каталогов не сохраняется ([config reference](https://v2.tauri.app/reference/config/)). Ресурсы читаются через `$RESOURCE`/`app.path().resource_dir()` (внутри `.app` — `Contents/Resources`; это известное поведение, прямой ссылки в этой справке нет).

### 2.4 `build.rs`

```rust
fn main() {
    tauri_build::build()
}
```
([шаблон](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/templates/app/src-tauri/build.rs)). Если нужны разрешения для своих команд — см. §3.

`tauri-build` копирует `externalBin` рядом с исполняемым файлом при сборке и отказывается собирать, если имя sidecar совпадает с именем Cargo-пакета (ошибка «Cannot define a sidecar with the same name as the Cargo package name») — [SRC tauri-build/src/lib.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-build/src/lib.rs). Следствие: пакет `app` не может иметь sidecar `app`; sidecar `helper` допустим, пока Cargo-пакет Tauri называется иначе (`app`).

### 2.5 `Cargo.toml` крейта `crates/app`

На базе шаблона [Cargo.crate-manifest](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/templates/app/src-tauri/Cargo.crate-manifest) + команды установки плагинов со страниц плагинов (target-условия для десктопных плагинов).

```toml
[package]
name = "app"
version = "0.1.0"
edition = "2024"
rust-version = "1.90"

[lib]
name = "app_lib"
crate-type = ["staticlib", "cdylib", "rlib"]   # как в шаблоне; для чисто десктопа достаточно rlib (см. «Не подтверждено»)

[build-dependencies]
tauri-build = "2.7"

[dependencies]
tauri = { version = "2.12", features = ["tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tauri-plugin-notification = "2.8"
tauri-plugin-shell = "2.6"          # только если sidecar запускается через плагин (см. §5)
ipc-proto = { path = "../ipc-proto" }

[target.'cfg(any(target_os = "macos", windows, target_os = "linux"))'.dependencies]
tauri-plugin-single-instance = "2.5"
tauri-plugin-autostart = "2.7"
tauri-plugin-updater = "2.14"
```

### 2.6 `lib.rs` / `main.rs`

`main.rs` — по шаблону ([SRC](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/templates/app/src-tauri/src/main.rs)):

```rust
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}
```

`lib.rs` — каркас (структура из шаблона; плагины — со страниц плагинов; single-instance должен быть зарегистрирован **первым** — [TD single-instance](https://v2.tauri.app/plugin/single-instance/)):

```rust
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle().plugin(tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    None::<Vec<&str>>,
                ))?;
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![/* команды, см. §4 */])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## 3. Capabilities и permissions

Источник: [TD capabilities](https://v2.tauri.app/security/capabilities/), [TD permissions](https://v2.tauri.app/security/permissions/), [DRS tauri-build AppManifest](https://docs.rs/tauri-build/2.7.4/tauri_build/struct.AppManifest.html).

- Файлы capabilities лежат в `capabilities/` рядом с `tauri.conf.json` (в нашем случае `crates/app/capabilities/`), формат JSON / JSON5 / TOML. Поля: `identifier`, `description`, `windows` (метки окон), `permissions`, `platforms` (`linux`, `macOS`, `windows`, `iOS`, `android`), `remote: {urls: [...]}` ([TD capabilities](https://v2.tauri.app/security/capabilities/)). На странице «capabilities-for-windows-and-platforms» также есть поле `local` ([TD](https://v2.tauri.app/learn/security/capabilities-for-windows-and-platforms/)); поле `webviews` в документации не найдено.
- По умолчанию включены **все** файлы из `capabilities/`; как только в `app.security.capabilities` (в `tauri.conf.json`) перечислены идентификаторы — действуют только они.
- **Свои команды `#[tauri::command]` по умолчанию не ограничиваются capabilities**: любая команда из `invoke_handler` доступна всем окнам. Чтобы включить ACL для своих команд, в `build.rs` перечисляют команды:

```rust
fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&["get_status", "toggle_chain", "check_proxy"]),
        ),
    )
    .unwrap();
}
```

  `AppManifest::commands` автогенерирует разрешения `allow-<command>` и `deny-<command>` (имя команды в snake_case). Дополнительно разрешения/наборы можно описать TOML-файлами в `permissions/` (`[[permission]] identifier = "..." commands.allow = ["..."]`, `[[set]]`; идентификаторы — только `[a-z]` и `-`, до 116 символов). В capability они указываются по идентификатору без префикса.

`capabilities/default.json` (основа — [шаблон](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-cli/templates/app/src-tauri/capabilities/default.json), остальное — из разрешений ниже):

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Главное окно",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-show",
    "core:window:allow-hide",
    "core:window:allow-set-focus",
    "core:window:allow-close",
    "notification:default",
    "autostart:allow-enable",
    "autostart:allow-disable",
    "autostart:allow-is-enabled",
    "updater:default",
    "allow-get-status",
    "allow-toggle-chain"
  ]
}
```

Что даёт `core:default`: наборы `core:app`, `core:event`, `core:image`, `core:menu`, `core:path`, `core:resources`, `core:tray`, `core:webview`, `core:window` (default-наборы). `show/hide/set-focus/close` в `core:window:default` **не входят** — добавлять явно ([TD core-permissions](https://v2.tauri.app/reference/acl/core-permissions/)). События: `core:event:allow-listen/unlisten/emit/emit-to` (там же).

Разрешения плагинов: уведомления — `notification:default` (все команды) либо `notification:allow-show` и т.п. ([TD notification](https://v2.tauri.app/plugin/notification/)); автозапуск — `autostart:allow-enable/disable/is-enabled` ([TD autostart](https://v2.tauri.app/plugin/autostart/)); обновление — `updater:default` (check, download, install, download-and-install) ([TD updater](https://v2.tauri.app/plugin/updater/)). У single-instance JS-API нет, разрешения не нужны.

Окно портала Wi-Fi (§7 ТЗ) показывает чужой сайт: не включать его метку в `windows` ни одной capability (вывод из того, что capability привязываются к меткам окон; прямое подтверждение — в «Не подтверждено»).

---

## 4. Команды и события

Источник: [TD calling-rust](https://v2.tauri.app/develop/calling-rust/), [TD calling-frontend](https://v2.tauri.app/develop/calling-frontend/), [DRS Emitter](https://docs.rs/tauri/2.12.5/tauri/trait.Emitter.html).

Правила (calling-rust): команда регистрируется в **одном** `generate_handler![...]` (повторные вызовы `invoke_handler` не накапливаются); в `lib.rs` команды не должны быть `pub`; ключи аргументов в JS — camelCase (`rename_all = "snake_case"` меняет это); типы аргументов/результата — `Serialize/Deserialize`; у `async`-команд нельзя брать заимствованные аргументы (`&str`) — только владеющие типы; для `State<'_, T>` в async-команде возвращать `Result<T, E>`; `Err` отклоняет промис в JS; события не типобезопасны, асинхронны, без возврата значения; `unlisten` вызывать после `await listen(...)`.

```rust
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
struct ChainState(Mutex<ipc_proto::Status>);

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[tauri::command]
fn get_status(state: State<'_, ChainState>) -> ipc_proto::Status {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
async fn toggle_chain(app: AppHandle, enable: bool) -> Result<(), AppError> {
    // ... вызов помощника по IPC ...
    app.emit("chain-changed", enable).map_err(|e| AppError::Other(e.to_string()))?;
    Ok(())
}

// Builder::default().manage(ChainState::default()).invoke_handler(tauri::generate_handler![get_status, toggle_chain])
```

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';

const status = await invoke<Status>('get_status');
await invoke('toggle_chain', { enable: true });   // ключи camelCase

useEffect(() => {
  const un = listen<boolean>('chain-changed', (e) => console.log(e.payload));
  return () => { un.then((f) => f()); };          // снимать при размонтировании
}, []);
```

События в Rust: трейты `Emitter` (`emit`, `emit_to`, `emit_filter`, реализован для `App`, `AppHandle`, `Window`, `Webview`, `WebviewWindow`) и `Listener` (`listen`, `once`, `unlisten`) — `use tauri::{Emitter, Listener}`; для адресной отправки — `EventTarget` (`emit_to("main", ...)`) ([DRS Emitter](https://docs.rs/tauri/2.12.5/tauri/trait.Emitter.html), [TD calling-rust](https://v2.tauri.app/develop/calling-rust/)). Для потоков данных рекомендован `tauri::ipc::Channel` (там же).

---

## 5. Трей, уведомления, автозапуск, single-instance

### Трей ([TD system-tray](https://v2.tauri.app/learn/system-tray/), [DRS TrayIcon](https://docs.rs/tauri/2.12.5/tauri/tray/struct.TrayIcon.html))

Нужна фича `tray-icon`. События мыши на Linux не поддерживаются. Меню по умолчанию открывается и левым, и правым кликом (`show_menu_on_left_click(false)` отключает).

```rust
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

// в setup:
let toggle = MenuItem::with_id(app, "toggle", "Включить", true, None::<&str>)?;
let ip = MenuItem::with_id(app, "ip", "IP: —", false, None::<&str>)?;
let reset = MenuItem::with_id(app, "reset", "Сбросить всё", true, None::<&str>)?;
let quit = MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?;
let menu = Menu::with_items(app, &[&toggle, &ip, &reset, &quit])?;

let tray = TrayIconBuilder::with_id("main-tray")
    .icon(app.default_window_icon().unwrap().clone())
    .menu(&menu)
    .show_menu_on_left_click(false)
    .on_menu_event(|app, event| match event.id.as_ref() {
        "quit" => app.exit(0),
        "reset" => { /* ... */ }
        _ => {}
    })
    .on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            if let Some(w) = tray.app_handle().get_webview_window("main") {
                let _ = w.unminimize(); let _ = w.show(); let _ = w.set_focus();
            }
        }
    })
    .build(app)?;

// смена иконки (зелёный/жёлтый/красный) и подсказки позже:
//   let t = app.tray_by_id("main-tray").unwrap();
//   t.set_icon(Some(Image::from_bytes(include_bytes!("../icons/tray-green.png"))?))?;   // нужна фича image-png
//   t.set_tooltip(Some("Защита включена"))?;
//   t.set_icon_as_template(false)?;   // только macOS
```

Подтверждено: `TrayIconBuilder::with_id` и `TrayIconBuilder::new()`, `.icon()`, `.menu()`, `.show_menu_on_left_click()`, `.on_menu_event()`, `.on_tray_icon_event()` — [TD system-tray](https://v2.tauri.app/learn/system-tray/); методы `TrayIcon::set_icon(Option<Image>)`, `set_icon_as_template(bool)` (macOS), `set_tooltip` (не работает на Linux), `set_title` (не на Windows), `set_menu`, `set_visible`, `id()` — [DRS TrayIcon](https://docs.rs/tauri/2.12.5/tauri/tray/struct.TrayIcon.html). `with_id`, `app.tray_by_id`, `Image::from_bytes` и фича `image-png` в процитированных страницах прямо не показаны (фичи `image-png`/`image-ico` существуют — crates.io API); см. «Не подтверждено».

### Уведомления ([TD notification](https://v2.tauri.app/plugin/notification/))

```rust
use tauri_plugin_notification::NotificationExt;
app.notification().builder().title("Project Z").body("VPN упал, kill switch включён").show()?;
```
```ts
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
let ok = await isPermissionGranted();
if (!ok) ok = (await requestPermission()) === 'granted';
if (ok) sendNotification({ title: 'Project Z', body: '…' });
```
Windows: «Only works for installed apps» — в dev-режиме уведомление показывается с именем и иконкой PowerShell. Actions API — только мобильные платформы.

### Автозапуск ([TD autostart](https://v2.tauri.app/plugin/autostart/))

```rust
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
app.handle().plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))?;
let m = app.autolaunch();
m.enable()?; let on = m.is_enabled()?; m.disable()?;
```
```ts
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
```
Страница показывает только вариант `MacosLauncher::LaunchAgent`; другие варианты не документированы там. Опциональный список аргументов передаётся приложению при старте.

### Single-instance ([TD single-instance](https://v2.tauri.app/plugin/single-instance/))

Регистрируется **первым** среди плагинов; замыкание получает `(app, args, cwd)` второго экземпляра. Нужны только десктопные платформы (`cargo add ... --target 'cfg(any(target_os = "macos", windows, target_os = "linux"))'`). Пример — в `lib.rs` выше.

---

## 6. Bundle: sidecar, ресурсы, universal, dmg, NSIS, подпись

### 6.1 `externalBin` (sidecar) ([TD sidecar](https://v2.tauri.app/develop/sidecar/))

- В `bundle.externalBin` указывается путь **без** суффикса, например `binaries/xray`; пути относительные — от каталога `tauri.conf.json`.
- На диске для каждой целевой платформы нужен файл с суффиксом `-<target-triple>` (+ `.exe` на Windows): `binaries/xray-aarch64-apple-darwin`, `binaries/xray-x86_64-apple-darwin`, `binaries/xray-x86_64-pc-windows-msvc.exe`. Triple текущей машины: `rustc --print host-tuple` (Rust 1.84+).
- В собранном приложении суффикс отбрасывается, бинарник лежит рядом с главным исполняемым файлом (в `.app` — `Contents/MacOS/`) ([SRC tauri-build](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-build/src/lib.rs) — `copy_binaries` убирает `-{target_triple}` из имени).
- **Universal macOS:** бандлер ожидает **универсальный** внешний бинарник с именем `...-universal-apple-darwin`, например `sqlite3-universal-apple-darwin` ([SRC bundler settings.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/settings.rs), комментарий к `external_bin`). Страница sidecar про universal молчит. См. «Не подтверждено» про необходимость одновременно и `-aarch64-…`, и `-x86_64-…`.
- Запуск из Rust: `app.shell().sidecar("xray")` (в `sidecar()` — только имя файла, без каталога) — требуется плагин `tauri-plugin-shell` и разрешение:

```json
{
  "identifier": "shell:allow-execute",
  "allow": [{ "name": "binaries/xray", "sidecar": true, "args": ["run", "-c", { "validator": "\\S+" }] }]
}
```
  (для `spawn()` — `shell:allow-spawn`; `args`: `true`/`false`/список статичных строк и `validator`-регулярок). Запуск **из Rust** разрешений capabilities не требует (они нужны для вызова из JS); в нашей архитектуре ядра запускает привилегированный помощник, а не UI, поэтому плагин shell может быть не нужен вовсе — `externalBin` тогда лишь способ доставить файлы в `.app` (по ТЗ §3 помощник и ядра затем копируются в каталоги root).
- `bundle.resources` — альтернатива для не-исполняемых файлов (конфиги, лицензии): см. §2.3.

### 6.2 Universal-сборка macOS ([TD cli reference](https://v2.tauri.app/reference/cli/))

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin --bundles app,dmg
```
`--target universal-apple-darwin` требует установленных обоих таргетов. `--bundles` принимает список, `--no-bundle` пропускает упаковку, `--ci`, `--no-sign`, `--skip-stapling` — флаги `tauri build`.

### 6.3 DMG и подпись ([config reference](https://v2.tauri.app/reference/config/), [TD sign/macos](https://v2.tauri.app/distribute/sign/macos/))

- `bundle.macOS.dmg`: `appPosition` (по умолчанию 180,170), `applicationFolderPosition` (480,170), `windowSize` (660×400), `background`, `windowPosition`.
- Ad-hoc: `"signingIdentity": "-"`. Док: на Apple Silicon подпись обязательна для приложений из интернета, ad-hoc-подпись **не** отменяет необходимости одобрения в «Конфиденциальность и безопасность». Нотаризация требует сертификата Developer ID (бесплатный аккаунт нотаризовать не может).
- `entitlements` — путь к `Entitlements.plist`; доп. ключи `Info.plist` — файл `Info.plist` рядом с `tauri.conf.json` (сливается с генерируемым) ([TD macos-application-bundle](https://v2.tauri.app/distribute/macos-application-bundle/)).

### 6.4 NSIS ([TD windows-installer](https://v2.tauri.app/distribute/windows-installer/))

Хуки (`installerHooks`, файл `hooks.nsh`):
```nsis
!macro NSIS_HOOK_PREINSTALL
  ; остановить службу перед копированием файлов
!macroend
!macro NSIS_HOOK_POSTINSTALL
  ; зарегистрировать службу/драйвер
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  ; снять правила WFP, остановить и удалить службу
!macroend
!macro NSIS_HOOK_POSTUNINSTALL
!macroend
```
`installMode`: `currentUser` (в `%LOCALAPPDATA%`, без админа) — по умолчанию; `perMachine` (в `C:\Program Files`, нужен администратор); `both` (выбор; установщик требует администратора). Установщик NSIS собирается на Windows (кросс-сборка с Linux/macOS возможна, но названа «last resort»). Режим установки WebView2: `bundle.windows.webviewInstallMode` (по умолчанию `downloadBootstrapper`).

---

## 7. Updater ([TD updater](https://v2.tauri.app/plugin/updater/), [SRC plugins-workspace updater](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/updater))

**Ключи:** подпись обязательна, отключить нельзя. Генерация: `npm run tauri signer generate -- -w ~/.tauri/projectz.key` (флаги: `-w/--write-keys`, `-p/--password`, `-f/--force`, `--ci` — [TD cli reference](https://v2.tauri.app/reference/cli/)). Публичный ключ — в `plugins.updater.pubkey` (**содержимое**, не путь). При сборке задать `TAURI_SIGNING_PRIVATE_KEY` (путь или содержимое) и при необходимости `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; `.env` для этих переменных не читается. Потеря приватного ключа = невозможность обновлять установленные копии.

**Конфиг:** `bundle.createUpdaterArtifacts: true`; `plugins.updater.endpoints` — массив URL (в релизе обязателен TLS; к следующему URL переход только при не-2xx), поддерживаются переменные `{{current_version}}`, `{{target}}` (`linux|windows|darwin`), `{{arch}}` (`x86_64|i686|aarch64|armv7`); `plugins.updater.windows.installMode`: `passive` (по умолчанию) / `basicUi` / `quiet` («quiet» не может сам запросить админа — подходит только для пользовательских установок). Также есть `dangerousInsecureTransportProtocol`. Пример конфигурации — §2.3.

**Артефакты:** macOS — `.app.tar.gz` + `.sig`; Windows — установщик NSIS/MSI + `.sig`; Linux — AppImage + `.sig`.

**Статический `latest.json`** (обязательны `version`, `platforms.<target>.url`, `platforms.<target>.signature`; Tauri валидирует файл целиком до сравнения версий, поэтому все перечисленные платформы должны быть полными; `signature` — **содержимое** `.sig`, не путь/URL):

```json
{
  "version": "1.2.0",
  "notes": "Список изменений",
  "pub_date": "2026-11-15T12:00:00Z",
  "platforms": {
    "darwin-aarch64":  { "signature": "<содержимое .sig>", "url": "https://github.com/OWNER/REPO/releases/download/v1.2.0/ProjectZ_universal.app.tar.gz" },
    "darwin-x86_64":   { "signature": "<тот же .sig>",      "url": "https://github.com/OWNER/REPO/releases/download/v1.2.0/ProjectZ_universal.app.tar.gz" },
    "windows-x86_64":  { "signature": "<содержимое .sig>", "url": "https://github.com/OWNER/REPO/releases/download/v1.2.0/ProjectZ_1.2.0_x64-setup.exe" }
  }
}
```
Формат ключа `OS-ARCH`. По исходникам плагина сначала ищется `{os}-{arch}-{bundle_type}` (например `windows-x86_64-nsis`), затем `{os}-{arch}` ([SRC updater.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/updater.rs), doc к `UpdaterBuilder::target`). Универсальный `.app.tar.gz` для обеих архитектур macOS прописывается под двумя ключами (`darwin-aarch64`, `darwin-x86_64`) — это вывод из схемы ключей, см. «Не подтверждено».

**Проверка и установка из Rust:**

```rust
use tauri_plugin_updater::UpdaterExt;

async fn check_and_install(app: tauri::AppHandle) -> tauri_plugin_updater::Result<()> {
    if let Some(update) = app.updater()?.check().await? {
        let mut downloaded = 0;
        update
            .download_and_install(
                |chunk, total| { downloaded += chunk; println!("{downloaded}/{total:?}"); },
                || println!("скачано"),
            )
            .await?;
        app.restart();
    }
    Ok(())
}
```
(официальный пример: [TD updater](https://v2.tauri.app/plugin/updater/)). Билдер: `app.updater_builder()` с `timeout`, `proxy`, `header`, `endpoints`, `pubkey`, `target`, `version_comparator`, `on_before_exit`.

**Раздельные шаги** (важно для §13 ТЗ — «скачивает UI, проверяет и ставит помощник»): по исходникам плагина есть `Update::download(on_chunk, on_finish) -> Result<Vec<u8>>`, `Update::verify(&[u8])` и `Update::install(bytes)`; страница плагина тоже упоминает «download() and install() separately» ([SRC updater.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/updater.rs)). Проверка подписи встроенная — по `pubkey` из конфига приложения; передачу байтов помощнику и собственную проверку на его стороне придётся писать самим.

**Windows:** установщик запускается через `ShellExecuteW(... "open" ...)`, после чего процесс приложения завершается (`std::process::exit(0)`) — [SRC updater.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/updater.rs). Повышение прав инициирует сам установщик: при `installMode: perMachine` (Program Files) — запрос UAC на каждое обновление; в режиме `quiet` он сам админа запросить не может ([TD updater](https://v2.tauri.app/plugin/updater/)).

**Новое в плагине (по исходникам, документировано слабо):** конфиг `requireSignedVersion` (по умолчанию `false`) — требует, чтобы подпись пакета была привязана к версии (подписи через `tauri signer sign --app-version`); пакеты, подписанные без версии, при включённом флаге отвергаются (`Error::MissingSignedVersion`) ([SRC config.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/config.rs), [updater.rs](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/updater/src/updater.rs)). Полезно против подмены «новая версия, старый подписанный пакет».

---

## 8. Прокси для webview и отдельное хранилище (портал Wi-Fi, §7 ТЗ)

Источники: [DRS WebviewWindowBuilder](https://docs.rs/tauri/2.12.5/tauri/webview/struct.WebviewWindowBuilder.html), [DRS WebviewBuilder](https://docs.rs/tauri/2.12.5/tauri/webview/struct.WebviewBuilder.html), [JSON-схема конфига](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-schema-generator/schemas/config.schema.json), [DRS wry WebViewBuilder](https://docs.rs/wry/latest/wry/struct.WebViewBuilder.html), [SRC wry](https://github.com/tauri-apps/wry).

| Возможность | Rust-метод | Схемы / ОС |
|---|---|---|
| Прокси | `.proxy_url(Url)` | Только `http://` или `socks5://`. **macOS: нужна cargo-фича `macos-proxy` у `tauri`, «компилируется только для macOS 14+»**; wry: «Requires macOS 14.0+ and the `mac-proxy` feature». Android/iOS не поддерживается (wry). Windows: wry добавляет в аргументы WebView2 `--proxy-server=http://…` / `socks5://…` |
| Отдельная папка данных | `.data_directory(PathBuf)` | В документации ограничений не указано; `data_store_identifier` описан как «replacement for data_directory not being available in WKWebView» — т.е. на macOS `data_directory` недоступен (WKWebView). В конфиге окна `dataDirectory` — путь относительно `localDataDir()` + метка окна |
| Хранилище по идентификатору | `.data_store_identifier([u8; 16])` | **macOS ≥ 14 и iOS ≥ 17**; Windows/Linux/Android — не поддерживается |
| Режим инкогнито | `.incognito(bool)` | Windows: WebView2 Runtime ≥ 101.0.1210.39, иначе молча ничего не делает. macOS/iOS: `nonPersistent` DataStore. Android — нет |

`proxyUrl`, `dataDirectory`, `dataStoreIdentifier`, `incognito` есть и в конфиге окна (`app.windows[]`) (JSON-схема). У `WebviewWindowBuilder` в docs.rs примечание о macOS у `proxy_url` отсутствует, но фича `macos-proxy` определена в `tauri` (`macos-proxy = ["tauri-runtime-wry?/macos-proxy"]` — crates.io API) и у `WebviewBuilder` примечание есть; считать ограничение общим.

Что важно из исходников wry для macOS ([wkwebview/mod.rs](https://github.com/tauri-apps/wry/blob/dev/src/wkwebview/mod.rs)): выбор хранилища — `incognito` имеет приоритет (`nonPersistentDataStore`); при заданном `data_store_identifier` и macOS ≥ 14 — `dataStoreForIdentifier`; иначе — **общее `defaultDataStore`** (т.е. на macOS 13 идентификатор молча игнорируется и cookie портала смешаются с хранилищем остальных окон приложения). Прокси на macOS применяется записью `proxyConfigurations` в data store (Network.framework `nw_proxy_config_create_http_connect` / `..._socksv5`), без проверки версии ОС в этом участке.

Пример окна портала:

```rust
use tauri::{WebviewUrl, WebviewWindowBuilder};

let portal = WebviewWindowBuilder::new(app, "portal", WebviewUrl::External("http://neverssl.com".parse()?))
    .title("Вход в сеть Wi-Fi")
    .proxy_url("http://127.0.0.1:18080".parse()?)     // портальный прокси помощника
    .incognito(true)                                   // отдельное хранилище, без следов на диске; работает на macOS 13 и Windows
    // .data_store_identifier(*b"projectz-portal!")   // macOS 14+: постоянное отдельное хранилище (16 байт)
    // .data_directory(app_data.join("portal"))        // Windows: отдельная папка WebView2
    .build()?;
```
Для сборки на macOS: `tauri = { version = "2.12", features = ["tray-icon", "macos-proxy"] }`.

---

## 9. Фронтенд: файлы-образцы (`ui/`)

База — актуальный официальный шаблон [create-vite `template-react-ts`](https://github.com/vitejs/vite/tree/main/packages/create-vite/template-react-ts) + рекомендации Tauri для Vite ([TD vite](https://v2.tauri.app/start/frontend/vite/): фиксированный порт, `clearScreen: false`, `envPrefix`, цели сборки `chrome105` на Windows и `safari13` на macOS; страница написана для Vite 8) + [Tailwind](https://tailwindcss.com/docs/installation/using-vite).

`ui/package.json`:
```json
{
  "name": "project-z-ui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.12.4",
    "@tauri-apps/plugin-autostart": "^2.7.0",
    "@tauri-apps/plugin-notification": "^2.8.1",
    "@tauri-apps/plugin-process": "^2.4.0",
    "@tauri-apps/plugin-updater": "^2.14.1",
    "react": "^19.3.1",
    "react-dom": "^19.3.1"
  },
  "devDependencies": {
    "@tailwindcss/vite": "^4.3.5",
    "@tauri-apps/cli": "^2.12.4",
    "@types/node": "^26.6.6",
    "@types/react": "^19.3.0",
    "@types/react-dom": "^19.3.0",
    "@vitejs/plugin-react": "^6.3.0",
    "tailwindcss": "^4.3.5",
    "typescript": "~7.0.2",
    "vite": "^8.3.8"
  }
}
```
(`@types/node` — как в шаблоне Vite, нужен для `process.env` в `vite.config.ts`.)

`ui/vite.config.ts`:
```ts
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { defineConfig } from 'vite'

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,                       // не затирать ошибки Rust
  server: {
    port: 5173,                             // = devUrl в tauri.conf.json
    strictPort: true,
    host: host || false,
    watch: { ignored: ['**/crates/**', '**/target/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // Tauri: Chromium на Windows (WebView2) и WebKit на macOS
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
})
```
(Блок `server/envPrefix/build` — со страницы [TD vite](https://v2.tauri.app/start/frontend/vite/); там `watch.ignored` указан как `**/src-tauri/**` — здесь заменён под нашу раскладку. Блок `hmr` для мобильных устройств опущен.)

`ui/tsconfig.json`:
```json
{
  "files": [],
  "references": [
    { "path": "./tsconfig.app.json" },
    { "path": "./tsconfig.node.json" }
  ]
}
```

`ui/tsconfig.app.json` (как в шаблоне Vite):
```json
{
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.app.tsbuildinfo",
    "target": "es2023",
    "lib": ["ES2023", "DOM"],
    "module": "esnext",
    "types": ["vite/client"],
    "allowArbitraryExtensions": true,
    "skipLibCheck": true,

    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",

    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"]
}
```

`ui/tsconfig.node.json` (как в шаблоне):
```json
{
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.node.tsbuildinfo",
    "target": "es2023",
    "lib": ["ES2023"],
    "types": ["node"],
    "skipLibCheck": true,

    "module": "nodenext",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true,

    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["vite.config.ts"]
}
```

`ui/src/index.css`:
```css
@import "tailwindcss";
```

`ui/index.html` — как в шаблоне Vite (`<div id="root">`, `<script type="module" src="/src/main.tsx">`), `ui/src/main.tsx`:
```tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
```

Версия Node: `^20.19.0 || >=22.12.0` (Vite 8 и `@vitejs/plugin-react` 6 — поле `engines` в npm).

---

## 10. Не подтверждено (нужна проверка на реальной сборке)

1. **Universal + sidecar: нужен ли полный набор имён.** Бандлер ожидает `<имя>-universal-apple-darwin` ([settings.rs](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.4/crates/tauri-bundler/src/bundle/settings.rs)), а `tauri-build` при компиляции каждой архитектуры берёт `externalBin` по `TARGET` (`-aarch64-apple-darwin`, `-x86_64-apple-darwin`). Вероятно, при universal-сборке нужны **все три** файла (lipo-склейка — наша), но в официальной документации это не описано.
2. **Подписывает ли Tauri sidecar-бинарники при ad-hoc подписи** (`signingIdentity: "-"`) — в прочитанных страницах не сказано. На Apple Silicon каждый Mach-O внутри `.app` обязан быть подписан; по ТЗ §13 подписывать ядра придётся и самим (`codesign -s -`) до упаковки.
3. **macOS 13 и прокси webview.** Документация: `proxy_url` на macOS — только 14+. Что произойдёт при запуске на macOS 13 бинарника, собранного с `macos-proxy` (в wry прокси ставится через KVC-ключ `proxyConfigurations` без проверки версии ОС) — **не проверено**, возможен сбой/исключение. Нужна явная проверка версии ОС и запасной путь из ТЗ (прямой доступ 80/443 с предупреждением).
4. **`data_directory` на macOS**: прямого текста «не поддерживается на macOS» нет; есть лишь формулировка в доке `data_store_identifier` («not available in WKWebView») и описание `dataDirectory` в схеме как «relative to localDataDir». На Windows (WebView2) использование ожидаемо, но платформенные оговорки в docs.rs отсутствуют.
5. **На macOS 13** `data_store_identifier` молча игнорируется (хранилище по умолчанию, общее с остальным приложением) — по исходникам wry; для изоляции cookie портала на macOS 13 остаётся `incognito(true)`. Работоспособность связки `incognito` + прокси на macOS 14 не проверялась.
6. **Окно портала и capabilities.** Документация говорит, что capability привязываются к меткам окон (и `remote.urls` для удалённого контента); подтверждения, что окно с внешним URL без capability не получает IPC, в прочитанных страницах нет — проверить, что из окна портала `window.__TAURI__`/`invoke` недоступны. Дополнительно можно ограничить навигацию `on_navigation`.
7. **Универсальный updater-артефакт для macOS**: схема ключей `OS-ARCH` подтверждена, но что один `.app.tar.gz` универсальной сборки нужно прописывать и под `darwin-aarch64`, и под `darwin-x86_64`, — вывод, прямого примера нет (tauri-action не проверялся).
8. **Обновление без UAC на Windows (§13 ТЗ).** API `download()/verify()/install(bytes)` в плагине есть, но `install` сам запускает установщик и завершает процесс (ShellExecuteW + exit). Схему «пакет скачивает UI, ставит служба SYSTEM» плагин не поддерживает «из коробки» — нужен собственный путь (скачать байты, передать помощнику, запустить установщик из-под службы). Документацией не подтверждено; решение записывать в `docs/` на этапе 6, как сказано в ТЗ.
9. **Трей:** `TrayIconBuilder::with_id`, `AppHandle::tray_by_id`, `Image::from_bytes` и точное имя фичи для загрузки PNG (`image-png`) — не найдены в прочитанных страницах (фича `image-png` в `tauri 2.12.5` есть по crates.io API). Сверить на docs.rs перед использованием. Поведение иконки-шаблона на macOS (`set_icon_as_template`) для цветных иконок (зелёный/жёлтый/красный) — template-иконки монохромные, для цветных `false`; не проверено визуально.
10. **Уведомления на macOS в dev-режиме** — страница про это молчит (оговорка есть только для Windows).
11. **`crate-type`**: в шаблоне `["staticlib","cdylib","rlib"]` (нужно для мобильных); для чисто десктопного приложения достаточно `rlib` — не проверено документацией.
12. **`tauri.conf.json` в `crates/app` и поиск CLI**: вывод про глубину 3 и переменные `TAURI_APP_PATH`/`TAURI_FRONTEND_PATH` сделан по исходникам CLI 2.12.4, в официальных страницах эти переменные не описаны; поведение `tauri build` с выходным каталогом (`target/release/bundle/` в корне workspace) — по общему устройству Cargo, не подтверждено ссылкой.
13. **TypeScript 7.0.2** — стабильная версия `latest`, шаблон Vite её использует (`~7.0.2`); совместимость сторонних типов/инструментов (ESLint-плагины и т.п.) не проверялась. Vite 8 по умолчанию собирает под браузеры «середины 2023» ([vite.dev/guide](https://vite.dev/guide/)); для macOS 13 (Safari/WebKit 16.x) безопаснее оставить явную цель `safari13`-стиля из рекомендаций Tauri либо проверить на Intel/macOS 13.
14. **Минимальная версия macOS для самого Tauri/WebKit:** страница webview-versions минимума не называет (в таблице самая старая — 10.13, `minimumSystemVersion` по умолчанию — 10.13). Для нас определяется ТЗ (13.0) и ядрами.
