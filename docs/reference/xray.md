# Справка по Xray-core (для Project Z)

Дата сверки: 2026-11-15. Назначение: генератор конфигов (`config-gen`) и помощник Project Z (см. `docs/TZ.md`, §3–§5).

## 0. Как читать и чем подтверждено

Источники (везде ниже на них ссылки):

- **Док** — официальный сайт https://xtls.github.io/ (английский раздел; русский раздел `https://xtls.github.io/ru/config/...` существует, содержимое по полям сверялось по английскому). Страницы сайта собираются из репозитория https://github.com/XTLS/Xray-docs-next (ветка `main`, `docs/en/config/...`); тексты читались оттуда напрямую. Через context7 использовались библиотеки `/websites/xtls_github_io` и `/xtls/xray-core` (список версий: v25.9.11, v25.12.8, v26.3.27, v26.6.1; v26.11.9 в нём **нет**).
- **Исходники** — теги `v26.11.9` и релизы GitHub XTLS/Xray-core, каталог `infra/conf/` (разбор JSON-конфига) и `transport/internet/`. Это главный арбитр: документация сайта относится к ветке `main` и местами опережает или отстаёт от релиза.
- **Проверено запуском** — `xray run -test` на машине (macOS 13.7.8, установлен **26.3.27**, не 26.11.9). Метка `[тест 26.3.27]` означает, что пример проходит на этой версии; на 26.11.9 пример **не запускался** (бинарника этой версии на машине нет), для него опора — исходники.

Метки: `[док]`, `[исх]` (исходник v26.11.9), `[тест]`. Всё, что не подтверждено, вынесено в §12.

Главное предупреждение: формат конфигов Xray меняется между версиями (в 2026 году удалены `allowInsecure`, `proxySettings`, добавлены `method`, `finalmask`, deprecated-предупреждения). Генератор писать под **зафиксированную** версию (согласуется с §3 ТЗ).

---

## 1. Версия, архивы, SHA-256, macOS

- Последний стабильный релиз на 2026-11-15: **v26.11.9** (опубликован 2026-11-09, `prerelease: false`, помечен latest). Страница: https://github.com/XTLS/Xray-core/releases/tag/v26.11.9 `[GitHub API releases/latest]`.
- Предыдущие: v26.10.31 (2026-10-31, срочный патч: по умолчанию выключен MPTCP), v26.10.21 (2026-10-21, срочный патч корневых сертификатов). v26.9.30 и v26.9.9 — prerelease. Номера версий: `год.месяц.день`.
- Release notes v26.11.9 — только две ссылки: транспорт XDRIVE (Google Drive) и «Chrome 155+ fingerprint by default». Это значит: **набор fingerprint по умолчанию в новом релизе меняется**; в конфиге всегда писать `fingerprint` явно.

Архивы (прямые ссылки; рядом лежит `.dgst` с тем же именем + `.dgst`):

| Платформа | Архив | Размер, байт |
|---|---|---|
| macOS Intel | https://github.com/XTLS/Xray-core/releases/download/v26.11.9/Xray-macos-64.zip | 16 509 590 |
| macOS Apple Silicon | https://github.com/XTLS/Xray-core/releases/download/v26.11.9/Xray-macos-arm64-v8a.zip | 15 319 055 |
| Windows x64 | https://github.com/XTLS/Xray-core/releases/download/v26.11.9/Xray-windows-64.zip | 16 697 596 |
| Windows ARM64 (если понадобится) | https://github.com/XTLS/Xray-core/releases/download/v26.11.9/Xray-windows-arm64-v8a.zip | 15 008 516 |

Контрольные суммы: файл `<архив>.dgst` содержит строки `MD5=`, `SHA1=`, `SHA2-256=`, `SHA2-512=` (формат проверен на `Xray-macos-arm64-v8a.zip.dgst`: `SHA2-256= 233f0c66681cd0726f81a6b2b6ed6a5710869e312047b522e46ed53035bebd26`). Брать SHA-256 из строки `SHA2-256=`. Для фиксации в репозитории значения нужно **вычислить самим** от скачанного архива и сверить с `.dgst`: `.dgst` лежит там же, где архив (тот же канал доверия). Значения остальных трёх архивов в этой справке намеренно не приведены — их не скачивали, а переписывать «по памяти» нельзя.

Требования к ОС:

- macOS: релиз собран Go 1.27.x (`go.mod` v26.11.9: `go 1.27`; в notes v26.10.21 указано «Go 1.27.3»). Go 1.27 требует **macOS 13 Ventura или новее** («Go 1.27 requires macOS 13 Ventura or later», https://go.dev/doc/go1.27, раздел Darwin). Вывод: Xray 26.11.9 работает на macOS 13+; на macOS 12 и старее — нет `[вывод из требований Go; сам запуск 26.11.9 не проверялся]`. Это пересекается с §2 ТЗ (если там указан macOS 12 — поднять или зафиксировать более старый Xray). На машине проверки (macOS 13.7.8) работает 26.3.27, собранный `go1.27.1`.
- Windows: требования Go 1.27 к Windows на странице релиз-нот не найдены (см. §12).
- Состав архива: бинарник `xray` + `geoip.dat`, `geosite.dat` (по докам `XRAY_LOCATION_ASSET`, §8). Нашему конфигу geo-файлы не нужны (правил с `geoip:`/`geosite:` нет).
- Тесты на машине: `xray` в PATH — это bash-обёртка Homebrew, выставляющая `XRAY_LOCATION_ASSET` и вызывающая настоящий бинарник.

Полезные подкоманды (`xray help`): `run` (по умолчанию), `version`, `api`, `convert`, `tls`, `uuid`, `x25519`, `wg`, `mldsa65`, `mlkem768`, `vlessenc`, `buildMphCache`.

---

## 2. Inbound `socks`

Док: https://xtls.github.io/en/config/inbounds/socks.html, общие поля inbound: https://xtls.github.io/en/config/inbound.html.

`settings`:

| Поле | Значение | По умолчанию | Источник |
|---|---|---|---|
| `auth` | `"noauth"` или `"password"` | `"noauth"` | [док] |
| `users` | массив `{ "user": "...", "pass": "..." }`; только при `auth: password` | пусто | [док] |
| `accounts` | **алиас `users`** в разборе конфига; в доке не описан | — | [исх `infra/conf/socks.go`], [тест 26.3.27] |
| `udp` | `true`/`false` — включить UDP ASSOCIATE | `false` | [док] |
| `ip` | IP, который inbound сообщает клиенту для UDP; по умолчанию локальный IP, принявший TCP-соединение | авто | [док] |
| `userLevel` | уровень политики | 0 | [док] |

Привязка и порт задаются **полями inbound**, не `settings`: `listen` (по умолчанию `"0.0.0.0"`, т.е. все интерфейсы!) и `port` [док inbound]. Для проекта обязательно `"listen": "127.0.0.1"`.

Замечания:

- Док предупреждает: socks не шифрует трафик; подходит для локального использования [док].
- При `auth: password` по доке к тому же inbound пароль требуется и для HTTP-запросов (смешанный режим) [док].
- Наблюдение [тест]: при запуске xray 26.3.27 на `127.0.0.1:10808` bind удался, хотя этот порт на машине уже слушал другой процесс (приложение «Tunnel»; lsof показал оба слушателя на TCP и UDP). Следствие: Xray не гарантирует ошибку при занятом порте (на macOS/Windows в исходниках есть `setReusePort`/`setReuseAddr`). Свободный порт помощник должен выбирать и проверять сам, а не полагаться на ошибку Xray. Причина (SO_REUSEPORT) в исходниках лишь предполагается, не доказана.
- Для sing-box-стороны: outbound `socks` sing-box → `127.0.0.1:PORT` с теми же `user/pass`; UDP через socks5 UDP ASSOCIATE работает, пока `udp: true`. Фактическая связка с sing-box не проверялась (§12).

---

## 3. Outbounds

Общий вид outbound: https://xtls.github.io/en/config/outbound.html — `sendThrough`, `protocol`, `settings`, `tag`, `streamSettings`, `mux`, `targetStrategy`. Тег, если задан, должен быть уникален.

- **Если ни одно правило routing не сработало, трафик уходит в первый outbound списка** [док outbound, routing]. Поэтому порядок outbound'ов важен, а при единственном правиле inbound → balancer это поведение — аварийное «куда попадёт всё остальное» (про unmatched-трафик см. §5).
- `targetStrategy` (outbound, не Freedom): резолвить ли **целевой** домен проксируемого запроса локально (по умолчанию `AsIs` — домен уходит на сервер как есть) [док outbound].

### 3.1 Формат `settings`: плоский и legacy

В документации v26.11.9 для vless/vmess/trojan/shadowsocks/hysteria `settings` — **плоские**: `address`, `port`, ... прямо в `settings` [док vless/vmess/trojan/shadowsocks/hysteria]. Разбор конфига при этом **принимает и старые формы** [исх `infra/conf/vless.go`, `vmess.go`, `trojan.go`, `shadowsocks.go`]:

- vless/vmess: `settings.vnext[0] = { address, port, users: [ { id, ... } ] }`;
- trojan/shadowsocks: `settings.servers[0] = {...}`;
- в каждом из `vnext`/`servers` должен быть **ровно один** элемент, и `users` — ровно один пользователь (иначе ошибка «should have one and only one member»; для нескольких серверов — несколько outbound'ов + balancer).

Проверено [тест 26.3.27]: vless во flat-формате и в `vnext`-формате проходят `-test`. Реальные подписки Happ могут приходить в legacy-форме, **генератор должен читать обе формы, писать одну (flat)**.

### 3.2 VLESS — https://xtls.github.io/en/config/outbounds/vless.html

```json
{ "protocol": "vless", "settings": {
    "address": "192.0.2.10", "port": 443,
    "id": "<uuid>", "encryption": "none", "flow": "xtls-rprx-vision",
    "level": 0, "reverse": { } } }
```

- `address` (домен/IPv4/IPv6), `port`, `id` (UUID или любая строка до 30 байт, отображается в UUIDv5), `level` [док].
- `encryption` — **обязательное, не пустое**; `"none"` отключает VLESS Encryption. Иначе строка вида `mlkem768x25519plus.native|xorpub|random.0rtt|1rtt.<padding>...<ключ>` (генерируется `xray vlessenc`) [док]. В разборе также есть `seed`, `testpre`, `testseed` — в доке не описаны [исх].
- `flow`: пусто; `xtls-rprx-vision` (перехватывает UDP/443 — QUIC); `xtls-rprx-vision-udp443` (не перехватывает). Vision работает с TCP+TLS/REALITY или с VLESS Encryption [док].
- `reverse` — обратный прокси; для нас не нужен (в белый список не включать).
- Ограничение: VLESS должен быть под внешним слоем безопасности (`tls`/`reality`); `security: none` допустим только для приватного адреса/доверенного канала или с VLESS Encryption [док].
- Релиз v26.1.23 добавил предупреждения об устаревании: VLESS без flow, VMess, Trojan, Shadowsocks (https://github.com/XTLS/Xray-core/releases/tag/v26.1.23). Это **предупреждения в логе**, конфиг проходит [тест 26.3.27: vmess/trojan/shadowsocks дают `[Warning] ... deprecated`, `Configuration OK`].

### 3.3 VMess — https://xtls.github.io/en/config/outbounds/vmess.html

`settings`: `address`, `port`, `id`, `security` (`aes-128-gcm` | `chacha20-poly1305` | `auto`, по умолчанию `auto`), `level`, `experiments` (`AuthenticatedLength`, `NoTerminationSignal`). Зависит от системного времени (допуск 120 с) [док]. [тест 26.3.27: vmess + ws + tls проходит.]

### 3.4 Trojan — https://xtls.github.io/en/config/outbounds/trojan.html

`settings`: `address`, `port`, `password` (обязательно), `email`, `level`. Требует TLS; `security: none` — только приватный адрес; в публичной сети по доке нужен ещё Mux, иначе TiT-детект [док]. [тест 26.3.27: trojan + grpc + tls проходит.]

### 3.5 Shadowsocks — https://xtls.github.io/en/config/outbounds/shadowsocks.html

`settings`: `address`, `port`, `method`, `password`, `email`, `level`. Методы: `2022-blake3-aes-128-gcm`, `2022-blake3-aes-256-gcm`, `2022-blake3-chacha20-poly1305` (рекомендуются; пароль — base64-ключ длиной 16 или 32 байта), а также `aes-256-gcm`, `aes-128-gcm`, `chacha20-poly1305` (`chacha20-ietf-poly1305`), `xchacha20-poly1305` (`xchacha20-ietf-poly1305`) [док]. [тест 26.3.27: проходит.]

### 3.6 Hysteria (v2) — https://xtls.github.io/en/config/outbounds/hysteria.html и https://xtls.github.io/en/config/transports/hysteria.html

Hysteria в Xray состоит из двух частей:

```json
{ "protocol": "hysteria",
  "settings": { "version": 2, "address": "192.0.2.1", "port": 443 },
  "streamSettings": {
    "network": "hysteria",
    "security": "tls",
    "tlsSettings": { "serverName": "h.example.com", "alpn": ["h3"] },
    "hysteriaSettings": { "version": 2, "auth": "<пароль>", "udpIdleTimeout": 60 } } }
```

- `settings.version` — только `2`; `address`, `port` обязательны [док].
- `hysteriaSettings`: `version` (2), `auth` (пароль, совпадает с серверным), `udpIdleTimeout` (сек, по умолчанию 60), `masquerade` (только сервер) [док].
- Транспорт `hysteria` требует `security: tls` (с REALITY и без безопасности не работает) [док transport.html]. Без транспорта `hysteria` UDP не проксируется [док outbound hysteria].
- Настройки скорости/brutal — в `finalmask.quicParams` (https://xtls.github.io/en/config/transports/finalmask.html), в этой справке не раскрыты (§12).
- [тест 26.3.27: проходит `-test`.]

### 3.7 `streamSettings`

Док: https://xtls.github.io/en/config/transport.html. Состав [док + исх `transport_internet.go`]:

- Транспорт: поле **`method`** (новое имя в доках) или **`network`** (старое) — оба принимаются, `method` приоритетнее [исх; тест 26.3.27 — оба проходят]. Значения (регистр не важен): `raw`/`tcp`, `xhttp`/`splithttp`, `kcp`/`mkcp`, `grpc`, `ws`/`websocket`, `httpupgrade`, `hysteria`, `masque`, `xdrive`. По умолчанию `raw`.
  - Удалены (ошибка): `h2`, `h3`, `http`, `quic` — замена XHTTP [исх].
  - Предупреждение «deprecated»: `grpc` (→ XHTTP stream-up H2), `ws`, `httpupgrade` (→ XHTTP H2/H3) [исх; тест 26.3.27].
  - Настройки транспорта: `rawSettings` (алиас `tcpSettings`), `xhttpSettings` (алиас `splithttpSettings`), `kcpSettings`, `grpcSettings`, `wsSettings`, `httpupgradeSettings`, `hysteriaSettings`, `masqueSettings`, `xdriveSettings` [исх].
- `security`: `none` | `tls` | `reality`; `xtls` — удалён [исх]. REALITY допустим **только** с `raw`, `xhttp`, `grpc` (иначе ошибка «REALITY only supports RAW, XHTTP and gRPC») [док + исх].
- `tlsSettings`, `realitySettings`, `finalmask`, `sockopt`.
- Для Freedom доступен только `sockopt` [док transport.html].
- Неизвестные поля **молча игнорируются** [тест 26.3.27: добавленное `sockopt.bogusField` не вызвало ошибки]. Следствие: опечатка в имени поля не будет поймана `xray run -test`; whitelist полей — задача генератора.

#### 3.7.1 `tlsSettings` — https://xtls.github.io/en/config/transports/tls.html

Клиентские поля (все по доке и `infra/conf/transport_security.go`):

| Поле | Смысл |
|---|---|
| `serverName` | имя сервера; SAN сертификата должен содержать значение; домен уходит в SNI, **IP-адрес SNI не отправляет**. IPv6 писать в `[]`. Спец-значение `"FromMitM"`. |
| `verifyPeerCertByName` | клиентское; имена (через `,`) для проверки сертификата вместо `serverName` |
| `pinnedPeerCertSha256` | SHA-256 сертификата (hex, через `,`); подменяет обычную проверку. Если хеш относится к CA — проверка имени идёт по `serverName` |
| `allowInsecure` | **удалён**: в `Build()` v26.11.9 безусловная ошибка; в 26.3.27 тот же результат [исх; тест] |
| `alpn` | массив; при включённом `fingerprint` по умолчанию `h2,http/1.1`, для `ws`/`httpupgrade` — `http/1.1` |
| `fingerprint` | uTLS-отпечаток (`chrome`, `firefox`, `safari`, `random`, ...); неизвестное имя — ошибка |
| `minVersion`, `maxVersion`, `cipherSuites`, `curvePreferences` | параметры TLS |
| `useSystemCA` (по умолчанию `false` — встроенный список CA), `disableSystemRoot`, `certificates` | корни доверия |
| `enableSessionResumption`, `masterKeyLog` | |
| `echConfigList`, `echSockopt` | клиентский ECH |
| `rejectUnknownSni`, `echServerKeys`, `certificates[]` | серверные |

**Значение `serverName` при пустом поле:** док: «When left empty, the value in `address` is used automatically if that value is a domain name» [док tls.html]. Если `address` — IP, имя не подставляется и SNI не отправляется. Для TLS-проверки сертификата без имени и без `pinnedPeerCertSha256`/`verifyPeerCertByName` соединение к IP не пройдёт проверку. **Вывод для проекта: при подстановке IP вместо домена всегда писать исходный домен в `tlsSettings.serverName`** (подтверждает §4 ТЗ).

#### 3.7.2 `realitySettings` (клиент) — https://xtls.github.io/en/config/transports/reality.html

Поля клиента:

| Поле | Смысл |
|---|---|
| `serverName` | одно из серверных `serverNames`. Можно указать произвольный **IP**: тогда ClientHello без SNI, а на сервере в `serverNames` должна быть пустая строка `""` |
| `fingerprint` | **обязательно**; для post-quantum (MLKEM) гарантированы `chrome`, `firefox`, `safari`; `unsafe`/`hellogolang` — ошибка |
| `password` | публичный ключ x25519 сервера (base64url, 32 байта); старое имя **`publicKey`** — принимается как алиас [исх; тест 26.3.27 — оба проходят] |
| `shortId` | hex, чётное число символов, до 16; пустая строка допустима, если у сервера есть `""` |
| `mldsa65Verify` | необязательный публичный ключ ML-DSA-65 |
| `spiderX` | начальный путь «краулера» |
| `show` | отладочный вывод |

Серверные поля (`target`/`dest`, `xver`, `serverNames`, `privateKey`, `minClientVer`, `maxClientVer`, `maxTimeDiff`, `shortIds`, `mldsa65Seed`, `limitFallbackUpload`/`limitFallbackDownload`) в клиентском конфиге **не использовать**: наличие `target`/`dest` заставляет ядро считать конфиг серверным, а `serverNames`/`shortIds` в клиенте дают ошибку (`non-empty "serverNames", please use "serverName" instead`) [док + исх].

**Значение `serverName` при пустом поле:** в исходнике `transport/internet/reality/reality.go` (v26.11.9): `if utlsConfig.ServerName == "" { utlsConfig.ServerName = dest.Address.String() }` — то есть берётся адрес из `address` outbound'а; в доке это поведение не описано [исх]. Если `address` — IP, то в SNI идёт IP (а fallback на сервере вернёт чужой сертификат) → **`serverName` обязателен**. Проверка сертификата в REALITY идёт по этому же имени (`DNSName: c.ServerName`) [исх].

#### 3.7.3 `xhttpSettings` — страница https://xtls.github.io/en/config/transports/xhttp.html пустая (одна ссылка на https://github.com/XTLS/Xray-core/discussions/4113)

Поля клиентского разбора по исходнику `infra/conf/transport_method.go`, структура `SplitHTTPConfig` (**в доке сайта не описаны**):

- Основные: `host`, `path`, `mode` (`auto` (по умолчанию, если пусто) | `packet-up` | `stream-up` | `stream-one`; иное — ошибка), `headers` (map строк), `extra` (JSON-блок с прочими полями; `host`/`path`/`mode` берутся из внешнего уровня), `downloadSettings` (отдельный `streamSettings` для скачивания), `h2Params`, `xmux`.
- Padding: `xPaddingBytes`, `xPaddingObfsMode`, `xPaddingKey`, `xPaddingHeader`, `xPaddingPlacement`, `xPaddingMethod`.
- Сессия/последовательность/загрузка: `uplinkHTTPMethod`, `sessionIDPlacement`, `sessionIDKey`, `sessionIDTable`, `sessionIDLength`, `seqPlacement`, `seqKey`, `uplinkDataPlacement`, `uplinkDataKey`, `uplinkChunkSize`.
- Флаги/лимиты: `noGRPCHeader`, `noSSEHeader`, `scMaxEachPostBytes`, `scMinPostsIntervalMs`, `scMaxBufferedPosts`, `scStreamUpServerSecs`, `serverMaxHeaderBytes`.
- `xmux`: `maxConcurrency`, `maxConnections`, `cMaxReuseTimes`, `hMaxRequestTimes`, `hMaxReusableSecs`, `hKeepAlivePeriod`. Если `xmux` пуст, по умолчанию `maxConnections 3`, `hMaxRequestTimes 600-900`, `hMaxReusableSecs 1800-3000`; одновременно задавать `maxConnections` и `maxConcurrency` нельзя [исх].

Назначение полей (кроме очевидных) в официальной документации не найдено → в whitelist генератора брать только поля, реально встречающиеся в подписках, остальные пропускать с записью в лог (согласно §5.1 ТЗ).

**Где задаётся host и что по умолчанию** (исходник `transport/internet/splithttp/dialer.go`, v26.11.9): `Host` в заголовке/URL запроса = `xhttpSettings.host`; если пусто — `tlsSettings.serverName`; если и он пуст — `realitySettings.serverName`; если и он пуст — **адрес назначения** (`dest.Address`, т.е. IP при подстановке IP) [исх]. Значит при подстановке IP достаточно задать `serverName` (TLS/REALITY) **или** `host`, но надёжнее явно писать `xhttpSettings.host` из подписки: в подписке `host` и SNI могут различаться (домен-фронтинг).

#### 3.7.4 Прочие транспорты (кратко)

- `rawSettings` (ex-tcp) — https://xtls.github.io/en/config/transports/raw.html (header `http`-обфускация); `wsSettings`: `path`, `host`, `headers`, `heartbeatPeriod`; `httpupgradeSettings`: `path`, `host`, `headers`; `grpcSettings`: `authority`, `serviceName`, `multiMode`, `idle_timeout`, `health_check_timeout`, `permit_without_stream`, `initial_windows_size`, `user_agent`; `kcpSettings`: `mtu`, `tti`, `uplinkCapacity`, `downlinkCapacity`, `cwndMultiplier`, `maxSendingWindow`, `header`, `seed` [исх `transport_method.go`; для gRPC док прямо пишет: Host не задаётся, писать домен в `address` или `ServerName` в `tlsSettings` — https://xtls.github.io/en/config/transports/grpc.html].
- Для `ws`/`httpupgrade` умолчание `host` при пустом значении в исходниках не проверялось (§12).

#### 3.7.5 `finalmask` — https://xtls.github.io/en/config/transports/finalmask.html

Последний слой маскировки, применяется после TLS/REALITY: `tcp: [ { type, settings } ]`, `udp: [...]`, `quicParams`. Типы TCP: `header-custom`, `fragment`, `sudoku`, `noise` и др. (полный список не сверялся). Схема `fragment` в доке:

```json
{ "type": "fragment", "settings": { "packets": "tlshello", "lengths": ["3-5"], "delays": ["10-20"], "maxSplit": "3-6" } }
```

В v26.11.9 разбор принимает и массивы `lengths`/`delays`, и скалярные `length`/`delay`; в **26.3.27** принимаются только `length`/`delay` (`lengths` дал ошибку `LengthMin can't be 0`) [исх; тест]. См. §4.2.

---

## 4. `sockopt`, `proxySettings`, Freedom

Док: https://xtls.github.io/en/config/transports/sockopt.html (поле `sockopt` внутри `streamSettings`).

### 4.1 Поля `sockopt` [док + исх `transport_sockopt.go`]

| Поле | Смысл / платформы |
|---|---|
| `interface` | привязка исходящего соединения к интерфейсу по имени. **Док: «Supported on Linux, iOS, Mac OS, and Windows»**. |
| `dialerProxy` | тег другого outbound'а, через который устанавливается нижележащее соединение (цепочки, фрагментация) |
| `domainStrategy` | `AsIs` (по умолчанию), `UseIP*`, `ForceIP*` (`IPv4`, `IPv6`, `IPv4v6`, `IPv6v4`) — как резолвить **адрес сервера** (для vless/vmess/trojan) или цель (для Freedom) |
| `mark` | SO_MARK, **только Linux**, нужен CAP_NET_ADMIN |
| `tcpFastOpen` | `true`/`false`/число; Linux 3.16+, macOS 10.11+ (на Windows в доке «реализация неверна», зачёркнуто). При отсутствии/0 — системное поведение |
| `tproxy` | `redirect`/`tproxy`/`off`, только Linux |
| `happyEyeballs` | `tryDelayMs`, `prioritizeIPv6`, `interleave`, `maxConcurrentTry`; только TCP, только при `domainStrategy` ≠ `AsIs`; **не работает вместе с `dialerProxy`** |
| `tcpKeepAliveIdle`, `tcpKeepAliveInterval`, `tcpUserTimeout`, `tcpMaxSeg`, `tcpWindowClamp`, `tcpcongestion`, `tcpMptcp`, `V6Only` | тонкая настройка TCP (часть — только Linux; MPTCP — только Linux 5.6+) |
| `addressPortStrategy` | SRV/TXT-подмена адреса/порта (не для Freedom) |
| `customSockopt` | произвольные setsockopt (`system`: linux/windows/darwin/android) |
| `acceptProxyProtocol`, `trustedXForwardedFor` | только inbound |

Нюансы по `interface` (из исходников v26.11.9):

- **macOS**: `IP_BOUND_IF` для IPv4 и `IPV6_BOUND_IF` для IPv6 по индексу интерфейса (`transport/internet/sockopt_darwin.go`). Неизвестное имя интерфейса → `BindError`, то есть соединение **не устанавливается** (fail-closed), а не уходит по умолчанию.
- **Windows**: `IP_UNICAST_IF` / `IPV6_UNICAST_IF` (`sockopt_windows.go`); неизвестное имя → `BindError`. В доке TUN для `autoOutboundsInterface` сказано, что на Windows такая привязка действует только при выключенном «weak host send» на физическом интерфейсе, иначе трафик может уйти в TUN и «зависнуть» (Xray сам включает/выключает это для автопривязки) — для ручного `sockopt.interface` это надо проверить на практике (§12).
- Имя интерфейса проверяется только при **дозвоне**, а не при `xray run -test`: конфиг с несуществующим именем проходит `-test` [тест 26.3.27].
- Имя интерфейса — имя системы (macOS: `en0`; Windows: имя адаптера из `net.InterfaceByName`, например «Ethernet»/«Wi-Fi»).

**`dialerProxy` + `interface`** (исходник `transport/internet/dialer.go`): если у outbound'а задан `dialerProxy`, он **не** делает системный дозвон сам, а перенаправляет соединение в outbound с этим тегом (`redirect(...)`); `interface` у «внешнего» outbound'а при этом не применяется. Привязка к интерфейсу должна стоять на **outbound-диалере** (в нашем случае freedom `fragment`) [исх]. Это уточняет §4 ТЗ: «все конечные outbound'ы» = те, кто реально вызывает системный dial; vless с `dialerProxy` — не конечный.

`domainStrategy` у внешнего outbound'а (с `dialerProxy`) выполняется **до** перенаправления [исх dialer.go].

### 4.2 `proxySettings` и `dialerProxy`

- `proxySettings: { "tag": "...", "transportLayer": ... }` — **старый** способ. В v26.11.9 `OutboundDetourConfig.Build()` возвращает ошибку «removed feature: outbound "proxySettings" → use `streamSettings.sockopt.dialerProxy`» [исх `infra/conf/xray.go`]. В 26.3.27 поле ещё работает (превращается в `dialerProxy`; при одновременном указании обоих — ошибка `proxySettings.tag is conflicted with sockopt.dialerProxy`) [тест]. **Вывод: ТЗ §5.1 упоминает «`dialerProxy`, `proxySettings`» как зависимости — зависимости надо искать в `sockopt.dialerProxy`; подписочный `proxySettings`, если встретится, конвертировать в `dialerProxy` на этапе разбора (не выдавать в Xray).**
- Док по `proxySettings` на сайте отсутствует (поиск по `docs/en/config` пуст).

### 4.3 Freedom — https://xtls.github.io/en/config/outbounds/freedom.html

`settings`: `redirect`, `userLevel`, `proxyProtocol`, `finalRules` (action `allow`/`block`, `network`, `port`, `ip`, `blockDelay`) [док]; в разборе также `targetStrategy`/`domainStrategy`, `fragment`, `noise`/`noises`, `ipsBlocked` (удалено, переехало в `finalRules`) [исх `infra/conf/freedom.go`]. `domainStrategy` Freedom по доке задаётся через `sockopt.domainStrategy`.

- **fragment (старая форма)**: `settings.fragment = { packets, length, interval }`: `packets` — `"tlshello"` или диапазон `"1-3"` (пусто = все); `length` и `interval` обязательны, `length` min ≠ 0; `packets` диапазон не может начинаться с 0 [исх]. В v26.11.9 печатается предупреждение об устаревании: заменить на `streamSettings.finalmask.tcp[type=fragment]` [исх `freedom.go`]. Конфиг при этом **работает** [исх; тест 26.3.27 — проходит без предупреждения].
- **noises**: `settings.noises = [ { type: "rand"|"str"|"hex"|"base64"|..., packet, delay, applyTo } ]` — UDP-шум Freedom [исх `freedom.go`; в доке `freedom.html` не описан; в finalmask есть тип `noise` — https://xtls.github.io/en/config/transports/finalmask.html]. [тест 26.3.27: `noises: [{type: rand, packet: "10-20", delay: "10-16"}]` проходит.]
- **finalmask-вариант fragment** для Freedom-диалера (новая форма): `"streamSettings": { "finalmask": { "tcp": [ { "type": "fragment", "settings": { "packets": "tlshello", "length": "100-200", "delay": "10-20" } } ] }, "sockopt": {...} }`, `settings: {}` [тест 26.3.27 с `length`/`delay` — проходит]. Работоспособность именно в роли `dialerProxy` на реальном сервере не проверялась (§12).
- Если у Freedom задан `sockopt.dialerProxy`, он перестаёт быть «конечным» и не применяет `finalRules` [док].
- Подписки Happ используют `fragment` во Freedom; поддерживать **старую форму на входе**, а выбор формы на выходе привязать к зафиксированной версии ядра.

---

## 5. Routing

Док: https://xtls.github.io/en/config/routing.html. Исходник: `infra/conf/router.go`.

`routing`: `domainStrategy`, `rules`, `balancers`, (`script` — Lua).

- `domainStrategy`: `AsIs` (по умолчанию, DNS не используется), `IPIfNonMatch`, `IPOnDemand`. Резолв нужен только при правилах с `ip` и доменной целью; встроенным DNS [док]. Для схемы проекта — `AsIs`.
- `rules[]`: поля `domain`/`domains`, `ip`, `port`, `sourcePort`, `localPort`, `network` (`tcp`, `udp`, `tcp,udp`), `sourceIP`/`source`, `localIP`, `user`, `vlessRoute`, `inboundTag`, `protocol` (`http`,`tls`,`quic`,`bittorrent`; нужен sniffing), `attrs`, `process` (только Windows/Linux), `localOS` (в разборе), `outboundTag`, `balancerTag`, `ruleTag`, `webhook` [док + исх].
- **`type: "field"` не обязателен.** В актуальном `RuleObject` поля `type` нет; в разборе `parseFieldRule` вызывается для любого правила без проверки `type` [исх]; [тест 26.3.27: правило без `type` и с `"type":"field"` — оба проходят].
- Условия внутри правила объединяются по **AND**; правила проверяются сверху вниз, срабатывает первое [док]. Нужно указать `outboundTag` **или** `balancerTag` (если оба — берётся `outboundTag`; если ни одного — ошибка при загрузке) [док + исх].
- **Если правил не совпало — трафик идёт в первый outbound** [док]. При единственном inbound'е правило `inboundTag → balancer` покрывает весь трафик inbound'а. Чтобы «первый outbound» не стал неожиданной точкой выхода, первым в списке ставить один из серверов подписки, а не freedom без привязки к интерфейсу. В примере первый — `srv-reality`.

### 5.1 Balancers

```json
{ "tag": "bal", "selector": ["srv-"], "strategy": { "type": "leastPing" }, "fallbackTag": "srv-reality" }
```

- `selector` — массив префиксов тегов outbound'ов (prefix match: `["a"]` ловит `a` и `ab`). **Следите, чтобы служебные теги (`fragment`) не попадали под префикс** [док].
- `strategy.type`: `random` (по умолчанию), `roundRobin`, `leastPing`, `leastLoad` [док]. `random`/`roundRobin` могут опираться на observatory и при заданном `fallbackTag` исключают недоступные узлы (без данных наблюдения считаются живыми); `leastPing`/`leastLoad` **обязательно** требуют observatory, узлы без наблюдения исключаются.
- `fallbackTag` — outbound, если по наблюдению недоступны все; без него выбирается outbound по умолчанию (первый) [док].
- `strategy.settings` (только `leastLoad`): `expected`, `maxRTT`, `tolerance`, `baselines`, `costs[ {regexp, match, value} ]` [док + исх].

### 5.2 Observatory — https://xtls.github.io/en/config/observatory.html

- `observatory`: `subjectSelector`, `probeUrl`, `probeInterval`, `enableConcurrency`.
- `burstObservatory`: `subjectSelector` (префиксы тегов) и `pingConfig`:

| Поле `pingConfig` | Значение | По умолчанию |
|---|---|---|
| `destination` | URL, возвращающий HTTP 204 | `https://connectivitycheck.gstatic.com/generate_204` |
| `connectivity` | URL проверки локальной сети; выполняется только после провала `destination`; пусто — не проверять | пусто |
| `interval` | **среднее** ожидаемое время между пробами одного outbound'а; минимум `10s` | `1m` |
| `sampling` | сколько последних результатов хранить | `10` |
| `timeout` | таймаут пробы | `5s` |
| `httpMethod` | метод пробы | `HEAD` |

Цикл проб = `interval × sampling`, внутри цикла проба в случайный момент; узел помечается неисправным за 1–2 цикла, восстанавливается после одной удачной пробы [док]. Пробы идут **через соответствующий outbound** (проверяется его соединение). Куда уходит запрос `destination` — на сервер Google через VPN-сервер подписки: это внешний трафик, отражающий выбор подписки, но не пользовательский; хорошо бы заменить URL на свой (поле меняется).

---

## 6. DNS

Док: https://xtls.github.io/en/config/dns.html, https://xtls.github.io/en/config/transports/sockopt.html (`domainStrategy`).

- Блок `dns` нужен для: (1) резолва при routing `IPIfNonMatch`/`IPOnDemand`; (2) резолва в outbound'ах при `domainStrategy`/`targetStrategy` ≠ `AsIs`; (3) перехвата DNS-трафика TUN/транспарентного прокси [док dns.html].
- **Без блока `dns`** (док sockopt.html): «If no DNSObject is configured, system DNS is used». При `domainStrategy: AsIs` Xray вообще не вызывает встроенный DNS: имя передаётся Go, который резолвит его через **системные настройки ОС** (для TCP — пробует IPv6/IPv4 по Happy Eyeballs Go; для UDP предпочитает IPv4) [док sockopt.html]. Это подтверждает §4 ТЗ: без подстановки IP адрес сервера резолвится системным DNS, в нашей схеме перехваченным TUN.
- Для proxy-outbound (vless и т.п.) `sockopt.domainStrategy` управляет резолвом **адреса сервера**; домен **целевого сайта** уходит на сервер как есть, пока `targetStrategy: AsIs` [док outbound/sockopt]. Поэтому при отсутствии блока `dns` Xray ничего не резолвит локально для пользовательского трафика.
- Важное исключение: резолвинг в `balancer`/routing с `domainStrategy: AsIs` не происходит; `burstObservatory` использует URL с доменом (`connectivitycheck.gstatic.com`) — домен идёт на сервер через outbound и резолвится там [вывод по `AsIs`; прямой проверки трафика не делали].
- «Local»-режимы DNS (`localhost`, `+local`) обращаются к системному DNS/Freedom, то есть сами бы дали утечку/петлю; в нашем конфиге их нет.
- Проверено: конфиг без блока `dns` проходит `-test` [тест 26.3.27].
- Адреса в `address` outbound'ов могут быть IPv6 (`[...]` для `serverName`).

---

## 7. TUN inbound в Xray и утверждения §3 ТЗ

Док: https://xtls.github.io/en/config/inbounds/tun.html.

Что умеет [док]:

- Создаёт TUN-интерфейс; трафик в него обрабатывает Xray. Поддерживаются **Windows, Linux, macOS, FreeBSD**. На Android/iOS fd передаётся приложением через `XRAY_TUN_FD`.
- `settings`: `name` (на macOS только `utunN`), `desc` (Windows), `mtu` (1500), `gateway` (адреса интерфейса), `dns` (**только Windows**), `userLevel`, `autoSystemRoutingTable` (авто-маршруты, Windows/Linux/macOS/FreeBSD), `autoOutboundsInterface` (авто-`sockopt.interface` для всех outbound'ов против петли; `"auto"`), `autoSystemDnsToGateway` (только Linux/FreeBSD), `autoSystemWfpBlockLeak` (только Windows, фильтры WFP против утечек DNS и пр.).

Проверка утверждений §3 ТЗ:

| Утверждение ТЗ | Вердикт |
|---|---|
| «в Xray появился TUN-inbound (Windows, Linux, macOS)» | **Подтверждено** (ещё и FreeBSD) [док] |
| «на macOS он не перехватывает системный DNS» | **Подтверждено**: «On macOS, the system DNS is not configured» и «on macOS, configure the system DNS yourself» [док]. Утечку DNS на macOS закрывает только внешняя настройка |
| «fake-ip в Xray нет» | **Опровергнуто частично**: в Xray есть **FakeDNS** (https://xtls.github.io/en/config/fakedns.html, `fakedns: { ipPool: "198.18.0.0/16", poolSize: 65535 }`) — но с предупреждением о «загрязнении» кэша ОС. Утверждение следует переформулировать: fake-ip есть, но не в том виде/не с теми гарантиями, что в sing-box, и без перехвата DNS на macOS — вывод остаётся в силе |
| «управляемого пула прокси с переключением в Xray нет» | Не опровергнуто: есть balancers (random/roundRobin/leastPing/leastLoad), но «пула пользовательских прокси с приоритетом и ручным переключением» как такового нет [док routing]; прямой проверки не было |
| Остальные требования §6–§8 (kill switch, брандмауэр) | Xray TUN их не закрывает: файрволл/kill switch в Xray отсутствуют; на Windows есть только WFP-фильтры утечек, на macOS ничего [док tun.html] |

Дополнительно (для обоснования, не для замены sing-box): `autoOutboundsInterface` — это ровно тот механизм «без петли», что в §4 ТЗ описан через ручной `sockopt.interface`; «Equivalent to automatically setting sockopt.interface for all outbounds ... which can be overridden by manually set sockopt.interface» [док tun.html].

---

## 8. Запуск, формат, окружение, логи

Исходник: `main/run.go`, `main/confloader/external/external.go`; команда `xray help run`.

- `xray run -c config.json` (команда `run` по умолчанию, поэтому `xray -c ...` тоже работает). Флаги: `-c`/`-config` (можно несколько — слияние), `-confdir <каталог>`, `-format <json|yaml|toml|auto>` (по умолчанию `auto`), `-test` (только проверить), `-dump` (напечатать слитый конфиг) [help; исх].
- **`xray run -test -c cfg.json`** печатает `Configuration OK.` и возвращает 0; при ошибке — `Failed to start: ...` и ненулевой код [тест 26.3.27].
- **stdin**: `-c stdin:` читает конфиг из стандартного ввода (`arg == "stdin:"`), формат по умолчанию `json` [исх; тест 26.3.27: `cat cfg.json | xray run -test -c stdin:` → `Configuration OK`]. Если `-c` не указан и на stdin подан pipe, в логе: «Using config from STDIN» [исх; тест: `cat cfg.json | xray run -test` → OK]. Так конфиг (с учётными данными) не попадает на диск; секреты не видны в argv (в отличие от `-c <путь>`, где путь виден, но секреты нет).
- Форматы: `json` (расширения `.json`, `.jsonc`), `yaml` (`.yaml`, `.yml`), `toml` [исх; тест 26.3.27: минимальные yaml и toml проходят]. Парсер JSON по умолчанию нестрогий — снимает комментарии и «лишние символы»; `XRAY_JSON_STRICT=true` включает стандартный RFC 8259 парсер (быстрее на больших конфигах) [док env.html].
- Переменные окружения [док https://xtls.github.io/en/config/env.html]: `XRAY_LOCATION_ASSET` (каталог `geoip.dat`/`geosite.dat`; по умолчанию `./`, `/usr/local/share/xray`, `/usr/share/xray`), `XRAY_LOCATION_CONFIG` (каталог `config.json`; по умолчанию каталог бинарника), `XRAY_LOCATION_CONFDIR`, `XRAY_JSON_STRICT`; прочие: `XRAY_LOCATION_CERT`, `XRAY_BUF_READV`, `XRAY_BUF_SPLICE`, `XRAY_VMESS_PADDING`, `XRAY_CONE_DISABLED`, `XRAY_RAY_BUFFER_SIZE`, `XRAY_BROWSER_DIALER`, `XRAY_XUDP_SHOW`, `XRAY_XUDP_BASEKEY`, `XRAY_TUN_FD` («см. исходники»). Часть переменных можно задать блоком `env` в самом конфиге, кроме `XRAY_JSON_STRICT`, `XRAY_LOCATION_CONFIG`, `XRAY_LOCATION_CONFDIR`, `GOGC`, `GOMEMLIMIT`, `GOMAXPROCS`, `GOTRACEBACK` — их нужно выставлять до запуска.
- Логи — https://xtls.github.io/en/config/log.html:
  - `logLevel` (`debug|info|warning|error|none`, по умолчанию `warning`) — так поле называется в доке и разборе; написание `loglevel` тоже принято на 26.3.27 [тест], но писать как в доке.
  - `access`, `error` — путь к файлу; пусто — stdout; спец-значение `none` — отключить.
  - `dnsLog` — лог DNS-запросов.
  - `maskAddress`: `quarter` | `half` | `full` или формат `/16+/32`: маскирует IP в логах (`1.2.*.*`, `1.*.*.*`, `[Masked IPv4]`; IPv6 `1234:5678::/32`, `1234::/16`, `[Masked IPv6]`). Принято в 26.3.27 [док; тест]. Маскируются именно **IP-адреса**; доменные имена, UUID и пр. нет.
  - Пользовательские адреса попадают в access-лог — для проекта: `access: "none"`, `logLevel: warning`, `maskAddress` включён.
- Для sing-box-стороны и приложения: сам Xray сообщает о запуске строкой `[Warning] core: Xray 26.3.27 started` (при `logLevel: warning`) [тест].

---

## 9. Полный пример под схему ТЗ

Файл проверен: `xray run -test -c final.json` → `Configuration OK.` на **26.3.27** [тест]; также при подаче через `-c stdin:`. Запуск боевой (без сети) показал, что socks слушает `127.0.0.1:10808` TCP+UDP [тест]. На v26.11.9 не запускался; поля выбраны так, чтобы они принимались и новой версией по исходникам (`network`, `password` в `realitySettings`, flat `settings`, freedom `fragment` с предупреждением об устаревании).

Все ключи, UUID и учётные данные — **сгенерированные плейсхолдеры** (`xray uuid`, `xray x25519`); адреса — из блоков документации RFC 5737 (`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`); имя интерфейса `en0` — подставляет помощник.

Что в примере соответствует схеме ТЗ:

1. `inbounds`: один socks на `127.0.0.1`, `auth: password`, `udp: true`.
2. `srv-reality` — VLESS Reality + raw (`flow: xtls-rprx-vision`), `address` — IP, `realitySettings.serverName` — явный домен.
3. `srv-xhttp` — VLESS + xhttp + TLS, `address` — IP, `tlsSettings.serverName` и `xhttpSettings.host` — явные.
4. `srv-frag` — vless c `dialerProxy: "fragment"`; сам дозвон делает Freedom `fragment`.
5. `sockopt.interface` — на **реальных диалерах**: `srv-reality`, `srv-xhttp` и freedom `fragment` (на `srv-frag` — нет, см. §4.1).
6. `routing`: единственное правило `inboundTag → balancerTag`; balancer `bal` по префиксу `srv-` (тег `fragment` в выборку не попадает), `leastPing`, `fallbackTag`.
7. `burstObservatory` на те же `srv-`; блока `dns` нет; `domainStrategy` — `AsIs`.

```json
{
  "log": { "logLevel": "warning", "access": "none", "maskAddress": "half" },
  "inbounds": [
    {
      "tag": "in-socks",
      "listen": "127.0.0.1",
      "port": 10808,
      "protocol": "socks",
      "settings": {
        "auth": "password",
        "users": [ { "user": "pz_user_PLACEHOLDER", "pass": "pz_pass_PLACEHOLDER" } ],
        "udp": true,
        "ip": "127.0.0.1"
      }
    }
  ],
  "outbounds": [
    {
      "tag": "srv-reality",
      "protocol": "vless",
      "settings": {
        "address": "192.0.2.10",
        "port": 443,
        "id": "4c56aecb-bdc2-4f74-930f-077140af72cb",
        "encryption": "none",
        "flow": "xtls-rprx-vision"
      },
      "streamSettings": {
        "network": "raw",
        "security": "reality",
        "realitySettings": {
          "serverName": "www.example.com",
          "fingerprint": "chrome",
          "password": "TJfUJHHGUwGh6hB5uly2TBesbikhPHMKtwdJPhsnkzQ",
          "shortId": "0123456789abcdef",
          "spiderX": "/"
        },
        "sockopt": { "interface": "en0" }
      }
    },
    {
      "tag": "srv-xhttp",
      "protocol": "vless",
      "settings": {
        "address": "198.51.100.20",
        "port": 443,
        "id": "2526ca37-89d1-409e-bfbd-328e82e0c647",
        "encryption": "none"
      },
      "streamSettings": {
        "network": "xhttp",
        "security": "tls",
        "tlsSettings": { "serverName": "cdn.example.com", "fingerprint": "chrome", "alpn": ["h2"] },
        "xhttpSettings": { "host": "cdn.example.com", "path": "/xh", "mode": "auto" },
        "sockopt": { "interface": "en0" }
      }
    },
    {
      "tag": "srv-frag",
      "protocol": "vless",
      "settings": {
        "address": "203.0.113.30",
        "port": 443,
        "id": "b937877e-a004-41a0-862f-1db8ec919466",
        "encryption": "none"
      },
      "streamSettings": {
        "network": "raw",
        "security": "tls",
        "tlsSettings": { "serverName": "frag.example.com", "fingerprint": "chrome" },
        "sockopt": { "dialerProxy": "fragment" }
      }
    },
    {
      "tag": "fragment",
      "protocol": "freedom",
      "settings": {
        "fragment": { "packets": "tlshello", "length": "100-200", "interval": "10-20" }
      },
      "streamSettings": { "sockopt": { "interface": "en0" } }
    }
  ],
  "routing": {
    "domainStrategy": "AsIs",
    "rules": [ { "inboundTag": ["in-socks"], "balancerTag": "bal" } ],
    "balancers": [
      {
        "tag": "bal",
        "selector": ["srv-"],
        "strategy": { "type": "leastPing" },
        "fallbackTag": "srv-reality"
      }
    ]
  },
  "burstObservatory": {
    "subjectSelector": ["srv-"],
    "pingConfig": {
      "destination": "https://connectivitycheck.gstatic.com/generate_204",
      "connectivity": "",
      "interval": "1m",
      "sampling": 3,
      "timeout": "5s",
      "httpMethod": "HEAD"
    }
  }
}
```

Замечания к примеру:

- Порядок `outbounds`: первым стоит `srv-reality` (получает трафик, если правило не сработало и как `fallbackTag`).
- `leastPing` обязан иметь наблюдение; без `burstObservatory` (или `observatory`) все узлы исключаются, и выбирается outbound по умолчанию [док routing].
- Пока наблюдения нет (первые секунды после запуска), трафик может идти в `fallbackTag`/первый outbound; задержку «первой пробы» учесть в состоянии «Запуск → проверка VPN» (§4.1 ТЗ).
- `access: "none"` отключает журнал доступа; в `maskAddress` значение `half` оставляет IPv4 вида `1.*.*.*`.
- `extra`, `downloadSettings`, `xmux` для `srv-xhttp` в пример не включены.

Вариант freedom-диалера на новой форме (finalmask), вместо `settings.fragment` (проверено на 26.3.27 только `-test`):

```json
{
  "tag": "fragment",
  "protocol": "freedom",
  "settings": {},
  "streamSettings": {
    "finalmask": { "tcp": [ { "type": "fragment", "settings": { "packets": "tlshello", "length": "100-200", "delay": "10-20" } } ] },
    "sockopt": { "interface": "en0" }
  }
}
```

---

## 10. Правила генерации конфига (выводы для `config-gen`)

1. Писать `settings` в плоском виде; читать также `vnext`/`servers`.
2. `serverName` (TLS и REALITY) всегда явный; `xhttpSettings.host` — из подписки либо равен `serverName`. Исходный домен сохранять отдельно от IP-подстановки.
3. Не писать `allowInsecure` (ошибка); пиннинг — `pinnedPeerCertSha256` и/или `verifyPeerCertByName`.
4. Не писать `proxySettings` (ошибка в 26.11.9) — только `sockopt.dialerProxy`.
5. `sockopt.interface` — на каждый outbound, реально вызывающий системный dial (в т.ч. freedom-диалер); не на outbound с `dialerProxy`.
6. `listen: "127.0.0.1"` у inbound обязательно (по умолчанию `0.0.0.0`).
7. Явно задавать `fingerprint` (в 26.11.9 изменён набор по умолчанию).
8. Неизвестные поля Xray молча принимает — whitelist и проверка схемы на стороне генератора; `xray run -test` проверяет только то, что ядро действительно разбирает.
9. Версия ядра зафиксирована; при смене — перепрогнать `xray run -test` на сгенерированных конфигах.

---

## 11. Список источников

- Релизы: https://github.com/XTLS/Xray-core/releases (v26.11.9, v26.10.31, v26.10.21, v26.2.6 — удаление `allowInsecure`, v26.1.23 — deprecated-предупреждения).
- Документация: https://xtls.github.io/en/config/ (inbounds/socks, inbounds/tun, outbounds/{vless,vmess,trojan,shadowsocks,hysteria,freedom}, outbound, transport, transports/{tls,reality,sockopt,finalmask,hysteria,grpc,raw,websocket,httpupgrade,mkcp}, routing, observatory, dns, fakedns, env, log), XHTTP: https://github.com/XTLS/Xray-core/discussions/4113.
- Исходники тега v26.11.9: https://github.com/XTLS/Xray-core/tree/v26.11.9 (`infra/conf/*.go`, `transport/internet/{dialer.go,sockopt_darwin.go,sockopt_windows.go,reality/reality.go,splithttp/dialer.go,tls/config.go}`, `main/run.go`, `main/confloader/external/external.go`).
- Go 1.27: https://go.dev/doc/go1.27.

---

## 12. Не подтверждено

1. SHA-256 архивов macos-64, windows-64, windows-arm64 — не вычислялись и не читались (только формат `.dgst` и значение для macos-arm64 из `.dgst`).
2. Запуск и `-test` на **v26.11.9** — бинарника нет; все примеры проверены на 26.3.27, различия 26.3.27 ↔ 26.11.9 учтены только по исходникам.
3. Минимальная версия Windows для Xray 26.11.9 (Go 1.27) — на странице релиз-нот не найдена. Минимальная macOS (13) — вывод из требования Go 1.27, а не из заявления авторов Xray.
4. Поведение `sockopt.interface` на Windows относительно «weak host send» для ручной привязки (док описывает только `autoOutboundsInterface`) — проверить на практике на этапе 0.
5. Реальная привязка к интерфейсу на macOS (`IP_BOUND_IF`) подтверждена только чтением кода; сетевой тест с трафиком не делался.
6. Назначение большинства полей `xhttpSettings` (кроме `host`, `path`, `mode`, `headers`, `extra`, `downloadSettings`, `xmux`) — официальной документации нет; перечень взят из кода.
7. Умолчание `host` для `ws`/`httpupgrade` при пустом поле — в исходниках не сверялось.
8. Работа freedom-диалера с `finalmask.fragment` в роли `dialerProxy` (в отличие от `settings.fragment`) на реальном сервере не проверялась; в v26.11.9 `settings.fragment` помечен устаревшим, срок удаления неизвестен.
9. Полный перечень типов `finalmask.tcp`/`udp` и параметры `quicParams` (brutal) не сверялись.
10. Совместная работа с sing-box (outbound socks → Xray, UDP через socks5 ASSOCIATE) не тестировалась.
11. Причина успешного bind на занятый порт (наблюдение §2) — предположительно SO_REUSEPORT, не доказана.
12. «Конфиг без блока `dns` не читает geo-файлы» и отсутствие обращений к DNS при `AsIs` в `burstObservatory` — не проверено трафиком.
13. Содержимое русского раздела сайта отдельно не вычитывалось (проверена доступность страниц; тексты — английские).
14. Список версий Xray в context7 не включает v26.11.9; сведения по ней — только из GitHub и сайта.
