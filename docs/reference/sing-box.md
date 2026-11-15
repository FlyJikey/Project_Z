# Справка по sing-box для Project Z

Дата сверки: 2026-11-15. Всё ниже сверено с документацией, а не написано по памяти.

Обозначения источников:
- **[site]** — https://sing-box.sagernet.org/ (страница указана рядом). Внимание: сайт показывает документацию ветки **1.15 (alpha)**, там есть поля, которых нет в стабильной 1.14.4 (например `version` у http-outbound, `dns_server_address`, `buffer_size`). Что относится только к 1.15, помечено.
- **[c7]** — context7, библиотека `/sagernet/sing-box/v1.14.4` (документация, привязанная к тегу v1.14.4; в ответах указан путь вида `docs/configuration/...md` в репозитории).
- **[src]** — исходный код тега v1.14.4 (https://github.com/SagerNet/sing-box/tree/v1.14.4), там, где документация молчит. Это вывод из кода, не из документации; помечено отдельно.
- **[gh]** — GitHub API релизов https://api.github.com/repos/SagerNet/sing-box/releases.

Всё, что не удалось подтвердить, собрано в последнем разделе «Не подтверждено».

---

## 1. Версия, архивы, миграция

### 1.1 Последняя стабильная версия

- **v1.14.4**, опубликована **2026-10-24T01:04:25Z**, не pre-release [gh]. Страница: https://github.com/SagerNet/sing-box/releases/tag/v1.14.4. Текст релиза: «Fixes and improvements» (подробностей нет).
- Предыдущие стабильные: 1.14.3 (2026-10-09), 1.14.2 (2026-09-24), 1.14.1 (2026-09-15) [gh].
- 1.15 существует **только как alpha** (последняя на момент сверки `v1.15.0-alpha.13`, 2026-10-24, pre-release) [gh]. Стабильного 1.15 нет. Следовательно, в ТЗ фраза «в 1.15 поле `stack` объявлено устаревшим» относится к alpha-ветке; для зафиксированной 1.14.4 `stack` не устарел (в доке v1.14.4 [c7] поле описано без пометки deprecated).

### 1.2 Архивы v1.14.4 и SHA-256 [gh]

SHA-256 берётся из поля `digest` ассета в GitHub API (`https://api.github.com/repos/SagerNet/sing-box/releases/tags/v1.14.4`). Отдельных файлов с контрольными суммами среди ассетов не обнаружено (в списке ассетов, отфильтрованном по `darwin`, `windows-amd64` и `checksum`, их нет; полный список ассетов не просматривался).

| Платформа | Файл | Размер, байт | SHA-256 |
|---|---|---|---|
| macOS Intel | https://github.com/SagerNet/sing-box/releases/download/v1.14.4/sing-box-1.14.4-darwin-amd64.tar.gz | 31503132 | `eae86b9b033bba50b7c44ab719e4bfac0aca05efd28483390ed3aa510f32db25` |
| macOS Apple Silicon | https://github.com/SagerNet/sing-box/releases/download/v1.14.4/sing-box-1.14.4-darwin-arm64.tar.gz | 29186204 | `0d8c7f5ed5d587b0f1a5e9772a708ec3bfa4ac4c9329604b01d8d9c2ba406838` |
| Windows x64 | https://github.com/SagerNet/sing-box/releases/download/v1.14.4/sing-box-1.14.4-windows-amd64.zip | 32891280 | `99185d75d314a07dbba4a130c8ad986cbeb629a07927975636b341ca61f991df` |

Есть также варианты `-legacy-macos-10.13` (darwin-amd64) и `-legacy-windows-7` (windows-amd64): их брать не нужно (macOS 13+ и Windows 10/11 по ТЗ §2).

Суммы получены из метаданных GitHub, сами архивы при сверке не скачивались и не хешировались локально. При фиксации в репозитории пересчитать `shasum -a 256` / `Get-FileHash` и сравнить.

### 1.3 Заметки о миграции (1.12 → 1.15)

Источники: https://sing-box.sagernet.org/migration/ , https://sing-box.sagernet.org/deprecated/ [site]; `docs/migration.md`, `docs/deprecated.md` v1.14.4 [c7]; список `Note` с версиями удаления в `experimental/deprecated/constants.go` v1.14.4 [src].

Страницы «1.13.0» в разделе Migration нет; для 1.13 есть только пункты удаления в Deprecated.

**Уже удалено (ломает конфиг, если встретится):**
- **1.13.0 удалены:** legacy special outbounds `block` и `dns`; legacy-поля inbound (по миграции: `sniff`, `sniff_timeout`, `domain_strategy` на inbound); поля override destination у direct-outbound; WireGuard outbound; `gso` у TUN; legacy ECH-поля [site /deprecated/].
- **1.14.0 удалён:** legacy-формат DNS-серверов (`"address": "tcp://1.1.1.1"` и т.п.) [site /deprecated/: «Old formats were removed in sing-box 1.14.0»].
- **1.12.0 удалены:** `inet4_address`, `inet6_address`, `inet4_route_address`, `inet4_route_exclude_address` и др. у TUN (объединены в `address`, `route_address`, `route_exclude_address`) [site /deprecated/]. Также удалены `geoip`/`geosite` в правилах (в пользу rule-set) [site /configuration/dns/rule/, /configuration/route/].

**Нужно сделать для 1.12 → 1.14 (то, что затрагивает наш генератор):**
1. DNS-серверы: вместо `"address": "..."` — типизированные: `{"type":"udp|tcp|tls|https|quic|h3","server":"1.1.1.1"}`, `{"type":"local"}`, `{"type":"dhcp","interface":"en0"}`, `{"type":"fakeip","inet4_range":...,"inet6_range":...}`. Fakeip теперь отдельный сервер, на него ссылается правило. `address_resolver` → `domain_resolver`, `strategy`/`client_subnet` уровня сервера → в правило или в `dns.strategy` [site /migration/#1.12.0].
2. `domain_strategy` в dial-полях outbound устарел с 1.12 → `domain_resolver: {"server":"local","strategy":"prefer_ipv4"}`; в `constants.go` v1.14.4 запланирован к удалению в 1.14.0 [src]. `outbound`-правила в DNS (`"outbound": "any"`) устарели в 1.12 → `domain_resolver` в outbound или `route.default_domain_resolver`.
3. Начиная с 1.14.0 `domain_resolver` обязателен для outbound/endpoint с доменным адресом сервера, если только не один DNS-сервер и не задан `route.default_domain_resolver` [site /configuration/shared/dial/]. **Проверено на 1.14.4:** при двух и более DNS-серверах ядро завершается ошибкой `missing route.default_domain_resolver or domain_resolver in dial fields is deprecated in sing-box 1.12.0 and will be removed in sing-box 1.14.0 ...` + `FATAL ... set environment variable ENABLE_DEPRECATED_MISSING_DOMAIN_RESOLVER=true`; её вызывает outbound без `detour` (в нашем случае `direct`), а не outbound с `detour` (см. §8.1). Решение для нас — не иметь outbound без `detour`.
4. Sniff, hijack и block переехали из inbound/outbound в **правила маршрута**: `{"action":"sniff"}`, `{"protocol":"dns","action":"hijack-dns"}`, `{"action":"reject"}` (см. §5) [c7 migration.md].

**Что изменилось в 1.14.0 (DNS):**
- Новые действия DNS-правил `evaluate` и `respond`, поле `match_response`, поля `response_rcode/answer/ns/extra` [site /configuration/dns/rule_action/, /rule/].
- Устарели (удаление запланировано на 1.16.0): `independent_cache`, `store_rdrc` (вместо него `store_dns`), legacy address-filter поля в DNS-правилах (`ip_cidr`, `ip_is_private` без `match_response`), `rule_set_ip_cidr_accept_empty`, опция `strategy` в действии DNS-правила [site /migration/, /deprecated/; [src] constants.go].
- Изменилось поведение `ip_version` и `query_type` в DNS-правилах: теперь они действуют и на внутренний резолв без указанного сервера; сочетание этих полей с legacy-фильтрами адресов и legacy `strategy` **отклоняется при старте** [site /migration/].
- TUN: новые `dns_mode` (`disabled`/`native`/`hijack`) и `dns_address` (since 1.14.0) [site; c7 tun.md].
- Новый тип «HTTP-клиент» (`route.default_http_client`, since 1.14.0) и `legacy-rule-set-download-detour` / `implicit-default-http-client` устарели; для нас не важны (rule-set не используем) [site /configuration/route/; src].

**1.15.0 (только alpha, в стабильную 1.14.4 не входит):** `stack` в TUN объявлен устаревшим, удаление запланировано на 1.17.0; sing-tun использует собственный TCP/IP-стек, нужно просто убрать `stack` [site /migration/, /deprecated/]. У http-outbound появились `version`, HTTP2/QUIC-поля (since 1.15.0) [site /configuration/outbound/http/]; в доке v1.14.4 их нет [c7].

---

## 2. TUN inbound

Источник: https://sing-box.sagernet.org/configuration/inbound/tun/ [site] и `docs/configuration/inbound/tun.md` v1.14.4 [c7]. Тип поля, если страница его не называет, выведен из примера.

### 2.1 Поля, значимые для ТЗ (macOS и Windows)

| Поле | Тип | Заметки |
|---|---|---|
| `type`, `tag` | string | `"tun"`, например `"tun-in"` |
| `interface_name` | string | Если пусто — выбирается автоматически. На Apple-клиентах NetworkExtension имя задаёт система (не наш случай) |
| `address` | массив строк | since 1.10.0. Префиксы IPv4/IPv6 интерфейса, напр. `172.19.0.1/30`, `fdfe:dcba:9876::1/126`. Заменил `inet4_address`/`inet6_address` (удалены в 1.12.0) |
| `mtu` | int | Значение по умолчанию на странице не указано |
| `auto_route` | bool | Ставит маршрут по умолчанию на TUN. «Чтобы избежать петли, задайте `route.auto_detect_interface` или `route.default_interface` или `outbound.bind_interface`» |
| `strict_route` | bool | См. §2.2 (на странице пометка «Changed in 1.13.3», что именно изменилось — не выяснено) |
| `route_address` | массив строк | since 1.10.0. Свои маршруты вместо маршрута по умолчанию при `auto_route` |
| `route_exclude_address` | массив строк | since 1.10.0. Исключаемые из TUN-маршрутов префиксы при `auto_route` |
| `route_address_set`, `route_exclude_address_set` | массив строк | Только Linux, требуют `auto_redirect` |
| `stack` | string | `system` / `gvisor` / `mixed`. В v1.14.4 не устарел. По умолчанию `mixed`, если собрано с тегом gVisor, иначе `system` [c7]. Устарел в 1.15 (alpha), удаление в 1.17.0 |
| `dns_mode` | string | since 1.14.0: `disabled` / `native` / `hijack`. По умолчанию `hijack`, если включён `auto_route`, иначе `disabled` |
| `dns_address` | массив строк | since 1.14.0. По умолчанию — следующий адрес после первых IPv4/IPv6 записей из `address`; соединения к нему обрабатываются как действие `hijack-dns` |
| `endpoint_independent_nat` | bool | Неэффективен с 1.11.0, использовать UDP-NAT поля (since 1.14.0). Не использовать |
| `include_interface`, `exclude_interface` | массив строк | Реализованы для Linux; на macOS/Windows страница их не обещает |
| `platform.http_proxy` | object | Для клиентских приложений Apple/Android; не наш случай |
| `gso` | bool | Удалён в 1.13.0 |
| Поля `auto_redirect*`, `iproute2_*`, `include_uid*`, `include/exclude_android_*`, `*_mac_address`, `exclude_mptcp`, `loopback_address`, `netns`, `multi_queue` | | Только Linux/Android (`multi_queue` — 1.15 alpha). На macOS/Windows не использовать |

Поле `udp_timeout` на странице TUN отсутствует (есть отдельные UDP NAT-поля: `udp_mapping`, `udp_filtering`, `udp_nat_max`, перенесены на страницу «UDP NAT Fields» в 1.14.0) [site]; `udp_timeout` есть как опция действия `route-options` (§5.3).

TUN inbound поддерживается только на Linux, Windows и macOS (в этом списке); без привилегий адреса и MTU не настраиваются автоматически [site].

### 2.2 Что делает `strict_route` (дословно по документации, пересказ)

Действует при `auto_route` [site; c7]:
- **Linux:** делает неподдерживаемые сети недоступными; без `strict_route` и `auto_redirect` весь ICMP идёт мимо TUN; с `auto_redirect` влияет на трафик `SO_BINDTODEVICE`.
- **Windows:** делает неподдерживаемые сети недоступными; предотвращает утечки DNS, вызываемые обычным multihomed-поведением резолвера Windows. Может нарушать работу некоторых приложений (пример в доке: VirtualBox).
- **macOS:** в описании `strict_route` **macOS не упоминается вовсе**. Это согласуется с ТЗ §7 («на macOS почти ничего не даёт»), но саму формулировку «почти ничего» документация не содержит; это вывод из отсутствия пункта.
- Дополнительно (`dns_mode`, since 1.14.0): на Windows `hijack` при включённом `strict_route` **блокирует трафик на порт 53 через интерфейсы, кроме TUN** [c7 tun.md].

### 2.3 Поведение `dns_mode` на macOS и Windows и конфликт с ТЗ

По докам `native` «устанавливает DNS интерфейса средствами платформы: per-interface DNS на Windows и Apple-платформах», а `hijack` = `native` + перехват порта 53 [c7 tun.md]. По умолчанию при `auto_route` включён `hijack`. То есть TUN в 1.14.x по умолчанию сам прописывает DNS своего интерфейса в системе (на macOS и Windows). Принцип ТЗ «системный DNS не меняем» вступает в конфликт со значением по умолчанию. Если нужно строго не трогать резолверы, в конфиге задать `"dns_mode": "disabled"` и перехватывать DNS правилом `hijack-dns` (§5). Как именно sing-box 1.14.4 ведёт себя при `disabled` на macOS/Windows (куда попадают запросы системного резолвера), документация не описывает. Проверить на этапе 0.

---

## 3. DNS

### 3.1 Формат (1.14.x)

Источник: https://sing-box.sagernet.org/configuration/dns/ , /dns/server/ , /dns/rule/ , /dns/rule_action/ [site]; context7 [c7].

Верхний уровень `dns`: `servers`, `rules`, `final` (тег сервера по умолчанию; если пусто — первый), `strategy` (`prefer_ipv4`/`prefer_ipv6`/`ipv4_only`/`ipv6_only`), `disable_cache`, `disable_expire`, `cache_capacity` (since 1.11.0, значения <1024 игнорируются), `reverse_mapping`, `client_subnet`. `independent_cache` устарел в 1.14.0 (удаление 1.16.0): не использовать (в официальном примере manual/proxy/client.md v1.14.4 он ещё встречается [c7] — копировать не надо).

`reverse_mapping`: сохраняет соответствие IP→домен после ответа DNS для маршрутизации; «на macOS может вызывать проблемы, так как DNS проксируется и кэшируется системой» [site /dns/].

### 3.2 fake-ip

- Сервер `{"type":"fakeip","tag":"fakeip","inet4_range":"198.18.0.0/15","inet6_range":"2001:2::/48"}`. Страница «since 1.12.0»; значения по умолчанию на странице не указаны (даны только примеры) [site /dns/server/fakeip/].
- Правило, отправляющее запросы на него: `{"query_type":["A","AAAA"],"server":"fakeip"}` (официальный пример; в 1.12+ тот же вид без `action` допустим, `route` — действие по умолчанию) [c7 migration.md, manual/proxy/client.md]. В нашем конфиге AAAA на fakeip не отправляем (см. §5.5).
- **Домен передаётся прокси без локального резолва.** Документация это явно не формулирует, но по исходнику v1.14.4 `route/route.go`: если адрес назначения входит в хранилище fakeip, он заменяется на FQDN из хранилища (`metadata.Destination = Socksaddr{Fqdn: domain}`, `metadata.FakeIP = true`); при отсутствии записи соединение завершается ошибкой `missing fakeip record, try enable experimental.cache_file` [src]. Дальше outbound `socks`/`http` получает доменное имя. Исходник — не документация; на этапе 0 подтвердить трассировкой (log `debug`: строка `found fakeip domain: ...`).
- Хранилище fakeip переживает перезапуск только при `experimental.cache_file.enabled: true` и `store_fakeip: true` [site /experimental/cache-file/; src]. Без этого после перезапуска sing-box старые fake-IP в кэше приложений превращаются в ошибки соединений.

### 3.3 Действие `hijack-dns` (маршрут)

`{"action":"hijack-dns"}` — «перехватывает DNS-запросы в DNS-модуль sing-box» [c7 route/rule_action.md]. В официальном примере ему предшествует `{"action":"sniff"}`, а матч — `{"protocol":"dns", "action":"hijack-dns"}` [c7 migration.md]. Это нужно, чтобы sniff определил протокол `dns` на TUN-соединении.

Кроме того, в 1.14.0 соединения на `dns_address` TUN обрабатываются как `hijack-dns` автоматически (если `dns_address` не задан); если задан — нужно своё правило `hijack-dns` [c7 tun.md].

### 3.4 DoH через outbound

Сервер `type: "https"` (since 1.12.0): `server` (обязательно; если домен — нужен `domain_resolver`), `server_port` (443), `path` (`/dns-query`), `headers`, `tls`, плюс dial-поля, включая `detour` [site /dns/server/https/]. Пример со ссылкой на outbound:

```json
{ "type": "https", "tag": "doh", "server": "8.8.8.8", "detour": "proxy" }
```

Без `detour` новый сервер использует «пустой direct outbound» (дословно по доке: по умолчанию эквивалентно пустому direct), т.е. напрямую в сеть [site /dns/server/https/]. Следовательно, `detour` для DoH задавать обязательно. Адрес сервера — IP, чтобы не требовался `domain_resolver` (иначе резолв имени DoH-сервера пойдёт отдельным запросом).

### 3.5 Действия DNS-правил (для справки)

[site /configuration/dns/rule_action/]: `route` (по умолчанию; `server`, `disable_cache`, `rewrite_ttl`, `timeout`, `client_subnet`, ...), `evaluate` (since 1.14.0), `respond` (since 1.14.0), `route-options`, `reject` (since 1.11.0; `method`: `default` = REFUSED, `drop`; `no_drop`), `predefined` (since 1.12.0; `rcode` по умолчанию `NOERROR`, `answer`/`ns`/`extra`).

---

## 4. Outbounds

### 4.1 socks

Источник: https://sing-box.sagernet.org/configuration/outbound/socks/ [site].

Поля: `server`, `server_port` (оба обязательны), `version` (`4` / `4a` / `5`; по умолчанию 5), `username`, `password` (пароль указан как SOCKS5), `network` (`tcp` или `udp`; по умолчанию включены оба), `udp_over_tcp` (отдельная страница UDP Over TCP, `false` или объект), dial-поля.

Про UDP по SOCKS5 и доменные имена на странице ничего не сказано. Подтверждено только то, что поле `network` по умолчанию разрешает и tcp, и udp. Работает ли UDP ASSOCIATE через цепочку `detour` → Xray socks → прокси пользователя, документация не сообщает; проверять на практике (см. «Не подтверждено»).

### 4.2 http

Источник: https://sing-box.sagernet.org/configuration/outbound/http/ [site], v1.14.4 [c7]. Поля: `server`, `server_port`, `username`, `password` (Basic), `path` («доступно только при `version: 1`», относится к 1.15), `headers`, `tls` + dial-поля. HTTP CONNECT-клиент. Поля `version`, `disable_version_fallback`, HTTP2/QUIC-поля — **since 1.15.0**, в 1.14.4 их нет. UDP через http-прокси невозможен (CONNECT только TCP): это общее свойство протокола, на странице sing-box не указано.

### 4.3 selector

Источник: https://sing-box.sagernet.org/configuration/outbound/selector/ , v1.14.4 [c7].

- `outbounds` (обязательное; теги), `default` (если пусто — первый), `interrupt_exist_connections` (при смене выбора обрывает существующие соединения; «затрагивает только входящие соединения, внутренние обрываются всегда»). Значение по умолчанию на странице не указано (в примере `false`) — в конфиге задавать явно `true`, как в ТЗ §6.
- «Селектором можно управлять только через Clash API».
- Запоминание выбора между запусками — `experimental.cache_file` (`store_selected` в Clash API с 1.8.0 устарел и включён по умолчанию при `cache_file.enabled`; «у целевых outbound должны быть теги») [site /experimental/clash-api/].
- Следствие для ТЗ: при перезапуске sing-box выбор из кэша может отличаться от `default`. Чтобы режим «Только основной» всегда начинался с №1, помощник должен после старта явно переключать selector через Clash API (или не включать `cache_file`... но `store_fakeip` требует его, поэтому — переключать).

### 4.4 direct, block, dns outbound

- `block` и `dns` outbound — устарели в 1.11.0, **удалены в 1.13.0**; заменены действиями `reject` и `hijack-dns` [site /deprecated/; c7 migration.md]. В 1.14.4 использовать только действия.
- `direct` outbound остаётся; поля override destination у него удалены в 1.13.0.

### 4.5 `detour` и dial-поля

Источник: https://sing-box.sagernet.org/configuration/shared/dial/ [site].

- `detour` — тег внешнего outbound; по докам при заданном `detour` «все остальные поля игнорируются» (имеются в виду остальные dial-поля).
- `domain_resolver` — since 1.12.0, изменено в 1.14.0. Строка (тег сервера) или объект как действие DNS-правила без `action`. С 1.14.0 обязателен для outbound с доменным адресом сервера, если не один DNS-сервер и нет `route.default_domain_resolver`.
- `domain_strategy` — устарел (1.12), см. §1.3.
- Остальные: `bind_interface`, `inet4_bind_address`, `inet6_bind_address`, `routing_mark` (Linux), `reuse_addr`, `connect_timeout`, `tcp_fast_open`, `tcp_multi_path`, `udp_fragment`, `network_strategy`/`network_type`/`fallback_*` (только клиенты Android/Apple), `netns` (Linux), `tcp_keep_alive*`, `disable_tcp_keep_alive`.
- **Из исходников v1.14.4** (`common/dialer/dialer.go`) [src] и проверкой `check`: если у outbound задан `detour`, обёртка локального резолва доменного адреса сервера создаётся только при явном `domain_resolver` у этого outbound. Конфиг с доменным `server` у прокси с `detour` и без `domain_resolver` проходит `check` (§8.2 (а)). Значит, имя прокси уходит в detour (Xray) как есть и резолвится на стороне цепочки (рантайм не проверялся). Для outbound «xray» (127.0.0.1) резолв не нужен. **Не задавать `domain_resolver` у прокси пользователя и не держать в конфиге outbound без `detour`** (например `direct`): именно он требует `default_domain_resolver` (§8.1).
- **Из исходников** (`route/network.go`) [src]: при `auto_detect_interface` привязка к интерфейсу для адреса назначения 127.0.0.1 выбирает интерфейс, которому принадлежит адрес (loopback), а не физический, так что соединение sing-box → Xray на loopback привязкой к Wi-Fi не ломается. Подтвердить на этапе 0 на macOS и Windows.

---

## 5. Route

### 5.1 Поля `route`

Источник: https://sing-box.sagernet.org/configuration/route/ [site].

`rules`, `rule_set`, `final` (тег outbound по умолчанию; если пусто — первый outbound), `auto_detect_interface` (Linux/Windows/macOS: привязывает исходящие соединения к интерфейсу по умолчанию, чтобы избежать петли под TUN; не действует при `outbound.bind_interface`), `default_interface` (Linux/Windows/macOS; не действует при `auto_detect_interface`), `default_mark` (Linux), `find_process`, `default_domain_resolver` (since 1.12.0), `default_network_strategy` и др. (клиенты Android/Apple). `geoip` и `geosite` удалены.

### 5.2 Формат правил и действий

Правило = условия + `action` (обязателен) [site /configuration/route/rule/, /rule_action/]. Условия, нужные нам: `inbound`, `protocol` (определён sniff), `network` (`tcp`/`udp`/`icmp`; icmp since 1.13.0), `ip_version` (4 или 6), `ip_is_private` (since 1.8.0), `ip_cidr`, `port`, `port_range`, `invert`, логические правила `{"type":"logical","mode":"and|or","rules":[...]}`. Поле `outbound` в правиле устарело в 1.11.0 (заменено действием `route` с `outbound`).

Действия [site /configuration/route/rule_action/]:
- `route` — `outbound` (обязательно) + route-options.
- `reject` — `method`: `default` (TCP RST для TCP, ICMP port unreachable для UDP; для ICMP echo — host unreachable) | `drop`; для ICMP echo ещё `reply` (изменено в 1.13.0). `no_drop` (по умолчанию false): если не включён, после 50 срабатываний за 30 с метод временно переключается на `drop`. Недоступно при `method: drop`.
- `hijack-dns` — без полей.
- `sniff` — `sniffer` (по умолчанию все), `timeout` (по умолчанию 300ms). Не финальное действие.
- `resolve` — резолв домена назначения (`server`, `strategy`, ...). Не финальное. В нашей схеме **не использовать**: локальный резолв ради маршрутизации нарушает «имя уходит прокси».
- `route-options` — `override_address`, `override_port`, `udp_timeout`, `udp_connect`, `tls_fragment` и др.
- `bypass` — since 1.13.0, только Linux с `auto_redirect`; не наш случай.

### 5.3 Порядок и отклонение IPv6 / UDP

Правила проверяются сверху вниз; `sniff` — не финальное, после него проверка продолжается [site].

- **Отклонение IPv6.** По исходнику v1.14.4 (`route/route.go`) [src]: `metadata.IPVersion` выставляется только если адрес назначения — IP (после подстановки fakeip назначение становится FQDN, и `ip_version` не задаётся). Поэтому:
  1. Основная защита — на уровне DNS: запросы типа AAAA отвечать пустым NOERROR (`predefined`), и **не задавать `inet6_range`** у fakeip. Приложение получит только A и пойдёт по IPv4.
  2. Страховка для прямых IPv6-литералов: правило маршрута `{"ip_version":6,"action":"reject"}`. На fakeip-соединения оно не сработает (см. выше), но fakeip-адресов IPv6 мы не выдаём.
  Это вывод из кода, не из документации; подтвердить трассировкой.
- **UDP.** Если прокси без UDP: `{"network":"udp","action":"reject"}` после правила `hijack-dns` (иначе будет отклонён и DNS по UDP/53). QUIC выборочно: `{"network":"udp","port":443,"action":"reject"}` (клиенты откатываются на TCP). Если UDP по SOCKS5 работает — не отклонять.
- `ip_is_private` + `direct` — по-умолчанию в официальных примерах; в ТЗ LAN разрешён опционально.

---

## 6. Clash API и cache_file

### 6.1 `experimental.clash_api`

Источник: https://sing-box.sagernet.org/configuration/experimental/clash-api/ [site], v1.14.4 [c7].

- `external_controller` — адрес RESTful API; если пусто — Clash API выключен. Пример `127.0.0.1:9090`. Unix-сокет на странице не упомянут.
- `secret` — токен: заголовок `Authorization: Bearer <secret>`. «Обязательно задавайте, если API слушает 0.0.0.0».
- `external_ui`, `external_ui_download_url`, `external_ui_download_detour` — для веб-панели; нам не нужны, не задавать (страница: при пустом каталоге `external_ui` скачивается панель по умолчанию).
- `default_mode` (по умолчанию «Rule»), `access_control_allow_origin` (since 1.10.0), `access_control_allow_private_network` (since 1.10.0).
- Устарели с 1.8.0 (перенесены в `cache_file`): `store_mode`, `store_selected`, `store_fakeip`, `cache_file`, `cache_id`. Не использовать.

**Эндпоинты.** На странице документации sing-box маршруты REST не перечислены (кроме `/ui`). Из исходников v1.14.4 [src] (`experimental/clashapi/server.go`, `proxies.go`):
- `GET /version`, `/configs`, `/proxies`, `/connections`, `/rules`, `/dns`, `/cache`, `/logs`, `/traffic` (последние через WebSocket).
- `GET /proxies/{name}` — информация; для selector есть `now` и `all`.
- **`PUT /proxies/{name}`**, тело `{"name":"<тег outbound>"}` — переключение selector. Успех — `204 No Content`; `400` с `"Must be a Selector"` (не selector) или `"Selector update error: not found"` (нет такого тега).
- **`GET /proxies/{name}/delay?url=<https-URL>&timeout=<мс>`** — проверка задержки; ответ `{"delay": <мс>}`; `504` при таймауте, `503` при ошибке теста, `400` если `timeout` не число. URL, начинающийся с `http://`, в коде заменяется на пустой (используется URL по умолчанию), поэтому давать **https**-URL.
- Авторизация: `Authorization: Bearer <secret>`; для WebSocket допускается `?token=<secret>`.
- Имя `{name}` — тег outbound, URL-кодировать.

Эти маршруты подтверждены только исходным кодом, не документацией sing-box (Clash-совместимый API формально описан в проекте Clash).

### 6.2 `experimental.cache_file`

Источник: https://sing-box.sagernet.org/configuration/experimental/cache-file/ [site]: `enabled`, `path` (по умолчанию `cache.db`), `cache_id`, `store_fakeip`, `store_rdrc` (устарел 1.14.0, удаление 1.16.0), `rdrc_timeout` (то же), `store_dns` (since 1.14.0), `buffer_size`, `flush_interval` (оба since 1.15.0, в 1.14.4 нет). Относительный `path` считается от рабочего каталога (`-D`); задавать абсолютный путь каталога, принадлежащего root (macOS: `/Library/Application Support/ProjectZ/...`).

---

## 7. Командная строка

Источник: https://sing-box.sagernet.org/configuration/ (раздел «Command», на сайте показан кратко: `check`, `format -w -c ... -D ...`, `merge output.json -c ... -D ...`) [site]; подробности флагов и поведение — из исходников v1.14.4 `cmd/sing-box/cmd.go`, `cmd_run.go`, `cmd_check.go`, `main.go` [src]; в документации они не описаны.

- `sing-box version` (бинарник из `cores/darwin-amd64`): `sing-box version 1.14.4`, `go1.26.10 darwin/amd64`, Revision `c005f35300ce76427dc285481ec5695f5b179cd2`, теги сборки: `with_gvisor, with_quic, with_dhcp, with_wireguard, with_utls, with_acme, with_clash_api, with_tailscale, with_ccm, with_ocm, with_cloudflared, with_naive_outbound, with_usbip, with_openvpn, with_openconnect, badlinkname, tfogo_checklinkname0`. Тег `with_clash_api` присутствует, `with_gvisor` присутствует.
- Глобальные флаги: `-c, --config <файл>` (можно повторять), `-C, --config-directory <каталог>` (читаются все `*.json`), `-D, --directory <каталог>` (рабочий каталог; создаётся при отсутствии), `--disable-color`. Если ни `-c`, ни `-C` не заданы — используется `config.json` в рабочем каталоге.
- `sing-box check [-c ...]` — разбирает и **создаёт экземпляр** (`box.New`), затем закрывает, не запуская сервисы. Ошибка → `log.Fatal(err)`: сообщение в stderr, ненулевой код выхода. Успех — тишина, код 0. Для `check` на `-c` нужно указывать тот же файл, что и для `run`. Поскольку проверка полная, она ловит устаревшие/удалённые поля и неизвестные теги.
- `sing-box run -c <файл>` — запуск. Перед стартом читает и объединяет конфиги, при ошибке — `log.Fatal`.
- **Чтение конфига из stdin: поддерживается и проверено на 1.14.4.** `-c stdin` читает конфиг из стандартного ввода (в документации не описано; в коде `readConfigAt`). `sing-box check --disable-color -c stdin < файл` для конфига из §8 — код выхода 0. При этом относительные пути (например `cache.db`) считаются от `-D`.
- **Формат ошибок (факт, 1.14.4, `--disable-color`).** Строка в stderr вида `FATAL[0000] <сообщение>`, код выхода 1, stdout пуст. Примеры:
  - неизвестное поле: `FATAL[0000] decode config at badfield.json: inbounds[0].foo: json: unknown field "foo"`
  - неверный тип: `FATAL[0000] decode config at ver_num.json: outbounds[0].version: json: cannot unmarshal number into Go struct field SOCKSOutboundOptions.version of type string`
  - битый JSON из stdin: `FATAL[0000] decode config at stdin: unexpected end of JSON input: row 2, column 0`
  - нет файла: `FATAL[0000] read config at nope.json: open nope.json: no such file or directory`
  - ошибка сборки объектов: `FATAL[0000] initialize outbound[4]: default domain resolver not found: nonexist`; `FATAL[0000] initialize DNS server[0]: default server cannot be fakeip`
  - устаревшая опция: сначала `ERROR[0000] <описание> is deprecated in sing-box <вер> and will be removed in sing-box <вер>, checkout documentation for migration: <ссылка>`, затем `FATAL[0000] to continuing using this feature, set environment variable ENABLE_DEPRECATED_<ИМЯ>=true`, код 1.
  Без `--disable-color` уровень обёрнут в ANSI-коды (`\x1b[31mFATAL\x1b[0m[0000] ...`). Число в скобках — время с запуска (`0000`). Успешный `check` ничего не печатает, код 0. Текст сообщений свободный: парсить только код выхода и искать подстроки.
- Предупреждения об устаревших опциях идут через менеджер deprecated в stderr (формат выше). Падение без переменной `ENABLE_DEPRECATED_*` подтверждено для `MISSING_DOMAIN_RESOLVER`; поведение для остальных устаревших опций (только предупреждение или тоже падение) не проверялось. Для CI: любой непустой stderr при `check` считать ошибкой.

---

## 8. Пример полного конфига (sing-box 1.14.4)

Назначение: TUN + fake-ip + selector из двух socks5-прокси, достигаемых через `detour` на socks-outbound `xray` (127.0.0.1) с авторизацией; Clash API; IPv6 отклоняется. Все секреты — плейсхолдеры (`__...__`), IP — из документационных диапазонов.

**Проверено ядром.** Файл из блока ниже проходит проверку на зафиксированном бинарнике (`sing-box version 1.14.4`, `darwin/amd64`, `cores/darwin-amd64/sing-box`), без вывода в stdout и stderr:

```
$ cores/darwin-amd64/sing-box check --disable-color -c final.json ; echo $?
0
$ cores/darwin-amd64/sing-box check --disable-color -c stdin < final.json ; echo $?
0
```

```json
{
  "log": {
    "level": "info",
    "timestamp": true
  },
  "dns": {
    "servers": [
      {
        "type": "https",
        "tag": "doh",
        "server": "8.8.8.8",
        "detour": "proxy"
      },
      {
        "type": "fakeip",
        "tag": "fakeip",
        "inet4_range": "198.18.0.0/15"
      }
    ],
    "rules": [
      {
        "query_type": [
          "AAAA"
        ],
        "action": "predefined",
        "rcode": "NOERROR"
      },
      {
        "query_type": [
          "A"
        ],
        "action": "route",
        "server": "fakeip"
      }
    ],
    "final": "doh",
    "strategy": "ipv4_only"
  },
  "inbounds": [
    {
      "type": "tun",
      "tag": "tun-in",
      "address": [
        "172.19.0.1/30",
        "fdfe:dcba:9876::1/126"
      ],
      "mtu": 1500,
      "auto_route": true,
      "strict_route": true,
      "stack": "system",
      "dns_mode": "disabled",
      "route_exclude_address": [
        "203.0.113.10/32",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "169.254.0.0/16",
        "fc00::/7",
        "fe80::/10"
      ]
    }
  ],
  "outbounds": [
    {
      "type": "socks",
      "tag": "xray",
      "server": "127.0.0.1",
      "server_port": 10808,
      "version": "5",
      "username": "__XRAY_SOCKS_USER__",
      "password": "__XRAY_SOCKS_PASSWORD__"
    },
    {
      "type": "socks",
      "tag": "proxy-1",
      "server": "198.51.100.11",
      "server_port": 1080,
      "version": "5",
      "username": "__PROXY1_USER__",
      "password": "__PROXY1_PASSWORD__",
      "detour": "xray"
    },
    {
      "type": "socks",
      "tag": "proxy-2",
      "server": "198.51.100.12",
      "server_port": 1080,
      "version": "5",
      "username": "__PROXY2_USER__",
      "password": "__PROXY2_PASSWORD__",
      "detour": "xray"
    },
    {
      "type": "selector",
      "tag": "proxy",
      "outbounds": [
        "proxy-1",
        "proxy-2"
      ],
      "default": "proxy-1",
      "interrupt_exist_connections": true
    }
  ],
  "route": {
    "rules": [
      {
        "action": "sniff"
      },
      {
        "protocol": "dns",
        "action": "hijack-dns"
      },
      {
        "ip_version": 6,
        "action": "reject"
      }
    ],
    "final": "proxy",
    "auto_detect_interface": true
  },
  "experimental": {
    "cache_file": {
      "enabled": true,
      "path": "/var/lib/projectz/sing-box-cache.db",
      "store_fakeip": true
    },
    "clash_api": {
      "external_controller": "127.0.0.1:9090",
      "secret": "__CLASH_API_SECRET__"
    }
  }
}
```

### 8.1 Почему в примере нет `direct` и `default_domain_resolver`

Первая версия примера содержала `direct` outbound (для LAN) и не проходила проверку:

```
ERROR[0000] missing `route.default_domain_resolver` or `domain_resolver` in dial fields is deprecated in sing-box 1.12.0 and will be removed in sing-box 1.14.0, checkout documentation for migration: https://sing-box.sagernet.org/migration/#migrate-outbound-dns-rule-items-to-domain-resolver
FATAL[0000] to continuing using this feature, set environment variable ENABLE_DEPRECATED_MISSING_DOMAIN_RESOLVER=true
```
(код выхода 1; так же через `-c stdin`). Выяснено экспериментами `check`:
- Ошибку вызывает именно **`direct` outbound** (у него нет `detour`, поэтому ядру нужен резолвер для доменных назначений) при наличии двух и более DNS-серверов. Если удалить `direct` из той же конфигурации, `check` проходит без `domain_resolver`. Удаление `detour` у DoH, `clash_api` или замена `domain_resolver` у DoH-сервера эффекта не дали.
- Исправление «добавить `route.default_domain_resolver`» тоже проходит `check` (проверено со значениями `doh` и `fakeip`; несуществующий тег даёт `FATAL ... default domain resolver not found: <тег>`). Но это включает локальный резолв для `direct`, и для нашей задачи это лишнее и опасное: `fakeip` как резолвер выдавал бы fake-адреса вместо реальных, а `doh` в итоге ходил бы через `proxy`, у которого селектор в свою очередь зависит от `xray` (риск круговой зависимости при старте).
- Поэтому в примере **`direct` убран совсем**. Раз нет outbound без `detour`, у sing-box нет пути, по которому он мог бы резолвить имя локально: ни одно имя не может быть разрешено мимо цепочки. LAN вместо правила `ip_is_private` → `direct` отдаётся ядру ОС: приватные префиксы указаны в `route_exclude_address` TUN (их пакеты вообще не попадают в TUN). Это соответствует опции «разрешить LAN»; при её выключении убрать приватные префиксы из списка (VPN-IP оставить). Побочный эффект: DNS на адреса LAN идёт мимо TUN и мимо `hijack-dns`; по ТЗ его блокирует брандмауэр.
- Если `direct` всё же понадобится, добавить `route.default_domain_resolver` с тегом отдельного DNS-сервера, которого нет в цепочке потребителей, и описать, зачем (в этой версии справки такой вариант не используется и не проверялся на утечки).

### 8.2 Результаты проверки вариантов ядром 1.14.4

Все варианты — правки конфига из §8 (то же ядро и команда `check --disable-color -c <файл>`), без `domain_resolver`/`default_domain_resolver`, без переменных `ENABLE_DEPRECATED_*`:

| Вариант | Результат |
|---|---|
| (а) у `proxy-1` `"server": "proxy1.example.net"` (домен) при `"detour": "xray"` | **проходит**, код 0, вывода нет |
| (б) `proxy-1` и `proxy-2` — `"type":"http"` вместо socks (без поля `version`) | **проходит**, код 0, вывода нет |
| (в) в `route.rules` после `hijack-dns` добавлено `{"network":"udp","action":"reject"}` | **проходит**, код 0, вывода нет |
| (а) + `route.default_domain_resolver: "doh"` | проходит, код 0 |
| Исходный пример с `direct` и доменным прокси | не проходит, та же ошибка `missing route.default_domain_resolver`, код 1 |

**Что `check` не доказывает.** Проверка валидирует и собирает объекты, но не открывает соединений. Что при доменном `server` прокси с `detour` локальный резолв не выполняется, подтверждено только чтением исходника (`common/dialer/dialer.go`: резолвер-обёртка создаётся для outbound с `detour` лишь при заданном у него `domain_resolver`) и тем, что `check` не потребовал резолвера. Рантайм-подтверждения нет (`run` без root не запускался). В конфиге без `direct` других путей локального резолва нет, поэтому имя прокси в таком случае уходит в `detour` (в Xray socks) как есть: это нужно проверить на этапе 0 трассировкой.

### 8.3 Пояснения к примеру

- Поле `version` у socks — **строка**: `"version": "5"` проходит, число `5` не проходит: `FATAL[0000] decode config at ver_num.json: outbounds[0].version: json: cannot unmarshal number into Go struct field SOCKSOutboundOptions.version of type string`. `"4a"` проходит. Поле можно опустить (по умолчанию 5).
- `stack`: в примере задан явно (`system`). Без `stack` конфиг тоже проходит. Релизный бинарник собран с тегом `with_gvisor`, значит по документации (`mixed`, если gVisor включён) значение по умолчанию — `mixed`; прямой проверкой через `check` это не наблюдается.
- `route_exclude_address`: `203.0.113.10/32` — плейсхолдер IP VPN-сервера (ТЗ §4); остальное — приватные префиксы (LAN).
- `dns_mode: "disabled"` — чтобы TUN не менял DNS системы (§2.3). Значение принимается; поведение в рантайме не проверялось.
- Если прокси без UDP, добавить в `route.rules` после `hijack-dns`: `{"network":"udp","action":"reject"}` (вариант (в), проходит).
- Без `inet6_range` fakeip не выдаёт IPv6; AAAA отвечается пустым NOERROR. Правило `ip_version: 6` + `reject` ловит только IPv6-литералы (§5.3).
- Путь `cache_file.path` в примере — каталог, которого может не быть: `check` его не проверяет (выяснится при `run`).
- Эндпоинты управления — §6.1. Переключение: `PUT http://127.0.0.1:9090/proxies/proxy` с `Authorization: Bearer __CLASH_API_SECRET__` и телом `{"name":"proxy-2"}`.

---

## 9. Не подтверждено

Закрыто запуском `check` / `version` на `cores/darwin-amd64/sing-box` (1.14.4): валидность примера §8 (проходит, код 0, без вывода); чтение конфига из stdin (работает); формат строки ошибки (§7); тип поля `version` у socks (строка); теги сборки (есть `with_gvisor`, `with_clash_api`); принятие вариантов (а), (б), (в) (§8.2). Остаётся неподтверждённым (всё, что требует `run`, root или документации):

1. **Рантайм-поведение примера §8** (`sing-box run` не запускался): что TUN поднимается, маршруты ставятся, `dns_mode: "disabled"` ведёт себя как ожидается.
2. **Эндпоинты Clash API** (`PUT /proxies/{name}`, `GET /proxies/{name}/delay`) — только по исходникам v1.14.4; документация sing-box их не описывает. На работающем ядре не проверялись.
3. **Передача домена прокси при fake-ip** (подстановка FQDN из хранилища fakeip) — по исходнику `route/route.go`; рантайм не проверялся.
4. **Отсутствие локального резолва имени прокси при `detour`** — по исходнику и по тому, что `check` не требует резолвера; сетевой трассировкой не проверено (§8.2).
5. **UDP по SOCKS5 через цепочку** `socks(proxy) → detour → socks(xray)`: документация молчит; `check` принимает конфиг, но работоспособность — только практикой. Вариант (в) ({"network":"udp","action":"reject"}) валиден, его семантика не исполнялась.
6. **Что делает `strict_route` на macOS** — документация молчит; «почти ничего не даёт» — вывод из отсутствия. Что изменилось в `strict_route` в 1.13.3 — не выяснено.
7. **`ip_version` и fakeip** (§5.3) — выведено из кода, не наблюдалось.
8. **Правило по порту 53 вместо `sniff`+`protocol: dns`** — документация даёт только вариант с sniff.
9. **`stack` по умолчанию**: у бинарника тег `with_gvisor`, значит по документации `mixed`; прямо не наблюдалось (`check` значение не печатает). Для Windows-бинарника теги не смотрелись.
10. **Windows:** `route_exclude_address` при `strict_route`, взаимодействие с `sockopt.interface` Xray — не покрыто; проверка Windows-бинарника не проводилась (запускать нельзя).
11. **Отдельные файлы контрольных сумм у релиза** не обнаружены; SHA-256 взяты из `digest` GitHub API (для darwin-amd64, по сообщению заказчика, совпал с архивом).
12. **Значение по умолчанию `interrupt_exist_connections`** — в документации не указано.
13. **Сценарий с `direct` outbound**: требует `route.default_domain_resolver`; какой сервер выбрать без утечки и круговой зависимости — не проверялось (в примере `direct` отсутствует, §8.1).
14. **Версия у http-outbound** в 1.14.4 отсутствует (проверено только то, что конфиг без `version` проходит); поведение поля `version` из 1.15 alpha не проверялось.
