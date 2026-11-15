# Справка: системная часть macOS и Rust-крейты для помощника

Статус: справочник к `docs/TZ.md` (§3, §4, §4.1, §7, §9, §10.1, §14). Составлен 2026-11-15 на машине macOS 13.7.8 (22H730), Intel x86_64.
Правило документа: у каждого утверждения есть источник; что подтвердить не удалось — в разделе «Не подтверждено» (§11).

## 0. Обозначения источников

| Метка | Что это |
|---|---|
| `[M:name]` | локальная man-страница этой машины (`man name`), macOS 13.7 |
| `[L:...]` | наблюдение на этой машине командой без root (команда указана) |
| `[SDK:path]` | заголовок из `/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/...` |
| `[crate:name-ver]` | исходники крейта с `static.crates.io` (прочитаны целиком, не по памяти) |
| `[crates.io]` | `https://crates.io/api/v1/crates/<имя>` (на 2026-11-15) |
| `[A:url]` | документация Apple по указанному URL |
| `[R:url]` | документация Rust / исходники rust-lang по URL |

Ничего не устанавливалось и не менялось; `rustc`/`cargo` на машине нет, поэтому **примеры кода на Rust не компилировались** (см. §11).
Команды `pfctl`, `launchctl bootstrap`, `dscl -create`, `systemsetup -set…` не запускались. Единственная «пробная» pf-команда — `pfctl -n` (только разбор, правила не загружаются), она работает без root.

---

## 1. pf: правила kill switch (ТЗ §7)

### 1.1. Факты о pf, на которые опирается схема

| Факт | Источник |
|---|---|
| Правила вычисляются по порядку; решает **последнее совпавшее**; если не совпало ни одно — пакет **пропускается** | `[M:pf.conf]` раздел PACKET FILTERING |
| `quick` — правило становится последним совпавшим, дальнейшая оценка прекращается | `[M:pf.conf]` параметр `quick` |
| `user <user>`: только пакеты сокетов, принадлежащих пользователю; **только TCP и UDP** (для других протоколов параметр игнорируется); учитывается *эффективный* uid на момент создания сокета; если процесс создал сокет под root и потом сбросил права — сокет остаётся «root» | `[M:pf.conf]` параметры `user`/`group` |
| `user` принимает имя **или число**. Имя должно существовать в момент разбора: `unknown user _projectz` при отсутствии пользователя; с числом (`user 299`) разбор проходит | `[L:]` `pfctl -a com.apple/zz.projectz -nvf pz2.conf` (число прошло); сообщение `unknown user` — `[L:]` `pfctl -nvf` с именем |
| Таблица `table <имя> persist` живёт, даже если на неё нет ссылок; без `persist` ядро удаляет таблицу вместе с последним правилом. `const` запрещает менять содержимое | `[M:pf.conf]` раздел TABLES |
| Таблица, объявленная без списка адресов, при загрузке `pf.conf` **не очищается**; объявленная с `{ }` — очищается | `[M:pf.conf]` раздел TABLES |
| Таблицы можно держать внутри anchor (приватные); правило в anchor берёт приватную таблицу, а если её нет — таблицу главного набора | `[M:pfctl]` описание `-a anchor` |
| Правила и таблицы, загруженные одним файлом, применяются **атомарно** | `[M:pf.conf]` TABLES (pf.conf: «loaded atomically») |
| Обновление таблицы: `pfctl -a <anchor> -t <имя> -T replace <адреса…>` (создаёт таблицу, если её нет; адреса можно брать из файла `-f`) | `[M:pfctl]` `-T replace`, пример `-a foo/bar -t mytable -T add` |
| Просмотр таблицы: `-T show`; список: `-s Tables`; флаги в `-vsTables`: `p` — persistent, `a` — активная, `r` — используется правилами | `[M:pfctl]` |
| `-E` включает pf и **увеличивает счётчик ссылок**; `-X token` снимает ссылку по токену; pf выключается только когда ссылок нет. `-s References` показывает pid/имя/токен/время | `[M:pfctl]` (`-E`, `-X`, `-s References`); `[L:]` `/etc/pf.conf` (комментарий: «each component … responsible for enabling and disabling PF via -E and -X») |
| `-a anchor` ограничивает `-f`, `-F`, `-s` указанным anchor; `-F all` сбрасывает всё внутри него | `[M:pfctl]` |
| `-s rules` — загруженные фильтр-правила; `-vsr` добавляет счётчики; `-s Anchors -a X` — anchor'ы под X; `-s info` — состояние и счётчики | `[M:pfctl]` |
| Без `-f`-файла в `-a`-режиме правила можно подавать со stdin: `pfctl -f-` (в man — пример `printf … \| pfctl -f-`) | `[M:pfctl]` |
| `pfctl -n` — «только разобрать, не загружать»; работает без root, печатает развёрнутые правила с `-v` | `[L:]` выполнено от обычного пользователя |
| Состояния (`keep state`) по умолчанию создаются `pass`-правилом; `no state` отключает | `[M:pf.conf]` STATEFUL TRACKING |
| `/dev/pf` доступен только root (`crw-------`), поэтому `pfctl -s …` без root не работает | `[L:]` `ls -l /dev/pf`, `pfctl -s info` → `Permission denied` |

Из этого следует важное для схемы:

* **Блокировка должна быть `quick` и стоять в конце anchor'а** (или все `pass` — `quick`, а общий `block` — тоже `quick` и последний). Тогда порядок правил = приоритет, и «последнее совпавшее» не может неожиданно перебить решение.
* **«Pass всё на utun» не нужно, если блокировка ограничена физическими интерфейсами.** Имя TUN заранее неизвестно: sing-box выбирает его сам, если `interface_name` пустой (`[A:https://sing-box.sagernet.org/configuration/inbound/tun/]`: «automatically selected if empty»), а брандмауэр по ТЗ §4.1 ставится *до* запуска ядер. Если правила `block … on $phys` не затрагивают `utunN`, то по `[M:pf.conf]` («no match → pass») трафик на utun проходит сам. Поэтому в наборе ниже правило для utun убрано; блок и разрешения заданы только на `$phys`.
* `user _projectz` в правилах надо писать **числом** (UID), потому что набор может загружаться раньше, чем пользователь создан/виден, и чтобы не зависеть от имени. UID берётся из записи пользователя при генерации правил.

### 1.2. Как подключить свой anchor, не меняя `/etc/pf.conf`

Содержимое `/etc/pf.conf` на этой машине `[L:]` `cat /etc/pf.conf`:

```
scrub-anchor "com.apple/*"
nat-anchor "com.apple/*"
rdr-anchor "com.apple/*"
dummynet-anchor "com.apple/*"
anchor "com.apple/*"
load anchor "com.apple" from "/etc/pf.anchors/com.apple"
```

`/etc/pf.anchors/com.apple` содержит только `anchor "200.AirDrop/*"` и `anchor "250.ApplicationFirewall/*"` `[L:]`.

Разбор по `[M:pf.conf]` (ANCHORS):

1. Правило `anchor "com.apple/*"` в главном наборе — **без `quick`, без `on`/`proto`/направления**: оно вычисляется для любого пакета.
2. Анкор со звёздочкой «означает: вычислить все anchor'ы, **непосредственно** прикреплённые в этой точке, **в алфавитном порядке** имён; вглубь рекурсивно не спускается».
3. Значит `com.apple/projectz` (или `com.apple/zz.projectz`) — прямой потомок `com.apple` — попадает под `anchor "com.apple/*"`. Имена `200.…` и `250.…` (цифры) сортируются раньше букв, т.е. наш anchor вычисляется после служебных Apple.
4. Правило с `quick` внутри anchor'а «final and abort the evaluation of the rules in other anchors and the main ruleset» (`[M:pf.conf]` ANCHORS) — наши `quick`-правила окончательны.
5. Главный набор больше ничего не содержит (после `anchor "com.apple/*"` — только `load anchor`), поэтому «последнее совпавшее» определяется нашими правилами.

Вывод: **схема «anchor внутри `com.apple/*`» по документации работает**: загрузка `pfctl -a com.apple/projectz -f -` не требует правки `/etc/pf.conf` и не трогает чужие anchor'ы. Это **теоретический** вывод из man; практическая проверка (пакет действительно блокируется, счётчики правил растут) нужна на этапе 0 — см. §11, п. 1. Тест без риска: загрузить в anchor только правило `block … log`‑free на заведомо неиспользуемый адрес/порт и посмотреть счётчики `pfctl -a com.apple/projectz -vsr`.

Оговорки:
* Любой `pfctl -f /etc/pf.conf` / `pfctl -F all` другой программы или системы может снести наш anchor (`-F all` в главном наборе сбросит и вложенные — `[M:pfctl]` описание `-F`; конкретно про вложенные anchor'ы в man явно не сказано → §11). Для этого и нужен «сторож правил» (§1.5).
* Имя anchor'а: ТЗ предлагает `com.apple/projectz`; любое имя с префиксом `com.apple/` сработает одинаково. Если нужно, чтобы наш anchor вычислялся **последним** среди Apple-овских — `com.apple/zz.projectz` (алфавитный порядок, `[M:pf.conf]`).
* Правила в anchor'е не могут содержать `set …` (опции pf), только фильтр-правила, макросы и таблицы; `set skip on lo0` недоступен — loopback пропускается явным `pass quick on lo0`. (Опции — часть главного набора; в man `set` описан как опция всего набора: `[M:pf.conf]` OPTIONS. То, что `-a` их отвергает, проверено только косвенно — см. §11.)

### 1.3. Полный набор правил (схема ТЗ §7)

Интерфейсы `$phys` — физические порты (en0 Wi-Fi, en1…) — подставляет помощник при генерации. Определение: `networksetup -listallhardwareports` перечисляет порты (`Wi-Fi → en0`, `Thunderbolt Bridge → bridge0`, …) `[L:]`; в динамическом хранилище у сервиса есть `InterfaceName` (см. §5.3).

```pf
# /Library/Application Support/ProjectZ/pf/projectz.rules  (грузится в anchor com.apple/projectz)
# Генерирует помощник. __UID__ — числовой UID служебного пользователя _projectz.

phys = "{ en0, en1 }"                      # физические интерфейсы (список подставляет помощник)

table <vpn_servers> persist                # IP VPN-серверов (только IPv4, см. примечание 3)
table <dns_servers> persist                # DNS-серверы текущей физической сети
table <lan> persist const { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, 224.0.0.0/4, 255.255.255.255 }
                                           # + помощник должен добавить реальную подсеть интерфейса, если она не RFC1918 (прим. 4)

# 1. Loopback: пропускать без состояния
pass quick on lo0 all no state

# 2. IPv6 на физических интерфейсах — блок в обе стороны
block drop quick on $phys inet6 all

# 3. Резолвер помощника → DNS-серверы сети (bootstrap-DNS). Правило ВЫШЕ блока DNS на LAN (прим. 2)
pass out quick on $phys inet proto { tcp, udp } from any to <dns_servers> port { 53, 853 } user __UID__

# 4. Xray → IP VPN-серверов
pass out quick on $phys inet proto { tcp, udp } from any to <vpn_servers> user __UID__

# 5. DHCP (клиент) — без состояния; ответ сервера разрешён отдельным правилом
pass out quick on $phys inet proto udp from port 68 to port 67 no state
pass in  quick on $phys inet proto udp from port 67 to port 68 no state

# 6. LAN (опция «разрешить локальную сеть»): DNS на LAN блокируем для всех, остальное пропускаем
block drop quick on $phys inet proto { tcp, udp } from any to <lan> port { 53, 853 }
pass out quick on $phys inet from any to <lan>
pass in  quick on $phys inet from <lan> to any

# 7. Всё остальное на физических интерфейсах — блок
block drop quick on $phys all
```

Проверка разбора `[L:]` (`pfctl -a com.apple/zz.projectz -nvf …`, без root, UID числом) — набор принимается; pfctl развернул `$phys` в правила для `en0` и `en1` отдельно, `proto { tcp, udp }` × `port { 53, 853 }` — в 4 правила на интерфейс.

Примечания:

1. **Порядок = приоритет**, т.к. все правила `quick`. Если «LAN» выключен — просто не включать три правила пункта 6 (последний `block quick` закроет всё).
2. **Противоречие/уточнение к ТЗ §7:** ТЗ говорит «DNS на адреса LAN всё равно блокируется». Но DNS-сервер сети часто **сам лежит в LAN** — на этой машине `[L:]` `scutil` → `State:/Network/Service/<id>/DNS` → `ServerAddresses: 192.168.0.1` (роутер). Поэтому правило резолвера (п. 3) обязано стоять **раньше** блока «DNS на LAN», иначе bootstrap-DNS не заработает. Блок касается всех остальных пользователей.
3. IPv6 заблокирован целиком (п. 2), значит, в `<vpn_servers>`/`<dns_servers>` бесполезны IPv6-адреса: резолвить только A-записи; сервер, у которого есть лишь AAAA, недостижим по этой схеме.
4. `<lan>` — выбор диапазонов по умолчанию. Если сеть отеля/оператора, например, `100.64.0.0/10` или публичная, роутер/DNS не попадёт в `<lan>`; помощник добавляет подсеть физического интерфейса (адрес+маска есть в `State:/Network/Service/<id>/IPv4`: `Addresses`, `SubnetMasks` `[L:]`).
5. `no state` для DHCP — потому что клиент шлёт с `0.0.0.0` на `255.255.255.255`, а ответ приходит с другого адреса; одно статeful-правило ответ не свяжет. (Рассуждение, а не цитата: man говорит лишь, что UDP-состояния сопоставляются по хостам и портам — `[M:pf.conf]`; рабочую проверку DHCP на этапе 0 → §11.)
6. ARP — не IP, pf его не фильтрует; в man это отдельно не сказано → §11.
7. Правило 4 (`user __UID__`, VPN) также позволит Xray делать DNS на `<dns_servers>` через правило 3 — «один пользователь на Xray и резолвер», как в ТЗ. Если нужно строго разделить, завести второго пользователя (`_projectz_dns`) для резолвера; ограничений в pf на это нет (`user` — любой пользователь).
8. **Хвост существующих соединений:** состояния, созданные до загрузки блока, продолжают пропускать пакеты. Сброс: `pfctl -F states` (сбрасывает **все** состояния системы, `[M:pfctl]` `-F states`) или точечно `pfctl -k <host>` (`[M:pfctl]` `-k`). Что именно удобнее для kill switch и нет ли `-k` по интерфейсу — проверить на этапе 0 (§11).

### 1.4. Загрузка, включение, обновление, снятие

```sh
# Загрузка набора (атомарно, вместе с таблицами), правила через stdin или файл
/sbin/pfctl -a com.apple/projectz -f /Library/Application\ Support/ProjectZ/pf/projectz.rules

# Включить pf со счётчиком ссылок; токен сохранить в снимок (атомарно, §8)
/sbin/pfctl -E            # печатает токен; точный формат вывода не подтверждён (§11)
/sbin/pfctl -s References # список ссылок: pid/имя/токен — можно восстановить токен после падения помощника

# Обновление таблиц (без перезагрузки правил)
/sbin/pfctl -a com.apple/projectz -t vpn_servers -T replace 203.0.113.10 203.0.113.11
/sbin/pfctl -a com.apple/projectz -t dns_servers -T replace 192.168.0.1
/sbin/pfctl -a com.apple/projectz -t vpn_servers -T show

# Снятие (в обратном порядке ТЗ §4.1): сначала свои правила и таблицы, потом ссылка на pf
/sbin/pfctl -a com.apple/projectz -F all
/sbin/pfctl -X <TOKEN>
```

Источники: `[M:pfctl]` (`-f`, `-a`, `-t/-T`, `-F`, `-E`, `-X`, `-s References`). Пустой список в `-T replace` и поведение `-F all` для вложенного anchor'а — см. §11. Абсолютный путь `/sbin/pfctl` — `[L:]` `which pfctl`.

### 1.5. «Сторож правил»

Что читать (всё от root; `[M:pfctl]`):

| Проверка | Команда |
|---|---|
| pf включён | `pfctl -s info` (строка `Status: Enabled`) |
| наша ссылка на pf жива | `pfctl -s References` (наш pid/токен) |
| наш anchor прикреплён | `pfctl -s Anchors -a com.apple` |
| правила на месте и не изменены | `pfctl -a com.apple/projectz -s rules` — сравнить с эталоном |
| таблицы заполнены | `pfctl -a com.apple/projectz -t vpn_servers -T show` (и `dns_servers`) |
| трафик реально бьётся о правила | `pfctl -a com.apple/projectz -vsr` — счётчики пакетов |

Эталон: сразу после загрузки сохранить вывод `-s rules` (после `-f`) и сравнивать строки. Эквивалентный «ожидаемый» текст даёт `pfctl -a com.apple/projectz -nvf rules` (формат развёрнутых правил совпадает по виду со `-s rules`, `[L:]`); идентичность форматов до последнего символа не подтверждена, поэтому надёжнее брать эталон из `-s rules` после реальной загрузки.

---

## 2. launchd: LaunchDaemon помощника

### 2.1. Пример plist

`/Library/LaunchDaemons/<Label>.plist` (имя файла = `Label`, `[M:launchd.plist]`: «expected convention … named `<Label>.plist`»).

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>org.example.projectz.helper</string>

  <key>ProgramArguments</key>
  <array>
    <string>/Library/PrivilegedHelperTools/org.example.projectz.helper</string>
    <string>--daemon</string>
  </array>

  <key>RunAtLoad</key>
  <true/>

  <key>KeepAlive</key>
  <true/>

  <key>ThrottleInterval</key>
  <integer>5</integer>

  <key>ProcessType</key>
  <string>Interactive</string>

  <key>StandardOutPath</key>
  <string>/var/log/projectz/helper.out.log</string>
  <key>StandardErrorPath</key>
  <string>/var/log/projectz/helper.err.log</string>

  <key>AssociatedBundleIdentifiers</key>
  <string>org.example.projectz</string>
</dict>
</plist>
```

Пояснения по ключам (все — `[M:launchd.plist]`):

| Ключ | Что известно |
|---|---|
| `Label` | обязательный; уникальный идентификатор |
| `ProgramArguments` | массив, первый элемент — путь к исполняемому; `Program` должен быть абсолютным путём. Это же `execvp`-аргументы: первым в массиве идёт имя программы |
| `RunAtLoad` | запуск при загрузке задания. Man советует избегать (ухудшает старт системы), но **`KeepAlive` его подразумевает**: «The use of this key implicitly implies RunAtLoad» — поэтому при `KeepAlive: true` отдельный `RunAtLoad` не обязателен |
| `KeepAlive` | `true` — безусловно держать; словарь условий (`SuccessfulExit`, `Crashed`, `PathState` …); «jobs that exit quickly and frequently … will be throttled». Для помощника ТЗ (§4.1: сам поднимается после падения) подходит `true`. Остановка «штатно» делается через `launchctl bootout`, а не `kill` |
| `ThrottleInterval` | по умолчанию задание запускается не чаще раза в 10 секунд |
| `ExitTimeOut` | время между SIGTERM и SIGKILL; **0 = бесконечно, нельзя**; значение по умолчанию «system-defined». Помощнику при остановке нужно успеть снять правила → задать явно (например 20) |
| `UserName`/`GroupName` | применимы только в system-домене. **Для помощника не задаём**: он должен быть root, а дочерние процессы получают права только через `setuid` в коде (§3.3). По вашему требованию «UserName для дочерних процессов — нет». Что умолчание — root, в man прямо не написано (§11) |
| `Sockets` | launch-on-demand сокеты: задание обязано получить дескрипторы через `launch_activate_socket(3)`; ключи `SockPathName`, `SockPathMode` (**в plist восьмеричные числа не поддерживаются — писать десятичные**: `0660` = `432`, `0600` = `384`), `SockPathOwner`, `SockPathGroup`. См. §2.2 |
| `MachServices` | для UI↔помощник не нужен (IPC — Unix-сокет, ТЗ §3) |
| `StandardOutPath`/`StandardErrorPath` | пути логов; каталог должен существовать, права на него — забота установщика. В логи — без секретов (ТЗ §14) |
| `AssociatedBundleIdentifiers` | связывает задание с приложением в «Фоновых элементах» (см. §2.4) |
| `ProcessType` | допустимые значения описаны в man; для помощника выбор `Interactive` — рекомендация против троттлинга, не требование |
| `Disabled` | может быть переопределён `launchctl enable/disable`; состояние хранится вне plist |

Права на сам файл plist: в man `launchd.plist` требований нет. На практике launchd отвергает plist не от root:wheel или с правом записи у группы/остальных — **в man не подтверждено** (§11); установщик должен ставить `root:wheel 0644`.

### 2.2. Sockets (launchd) или собственный сокет

* `Sockets` + `SockPathName`: launchd создаёт сокет заранее и запускает помощника по требованию (`[M:launchd.plist]` Sockets).
* Минусы для нашей задачи: (1) помощник по ТЗ должен работать **постоянно** (kill switch, сторож) — выгода launch-on-demand мала; (2) дескрипторы надо забирать через `launch_activate_socket(3)` — это C-функция libSystem; в крейте `libc` её наличие не проверялось (§11); (3) права, владелец и проверка uid всё равно нужны в коде.
* **Рекомендация:** собственный `bind` в коде помощника (§4) — полный контроль над правами и удалением устаревшего файла сокета.

### 2.3. Команды управления

Из `[M:launchctl]` (раздел SUBCOMMANDS):

```sh
# Загрузить в system-домен (нужен root; plist root:wheel)
launchctl bootstrap system /Library/LaunchDaemons/org.example.projectz.helper.plist

# Выгрузить (по label или по пути)
launchctl bootout system/org.example.projectz.helper
launchctl bootout system /Library/LaunchDaemons/org.example.projectz.helper.plist

# Запустить сейчас / перезапустить (-k убивает работающий экземпляр, -p печатает PID)
launchctl kickstart -kp system/org.example.projectz.helper

# Состояние службы (формат вывода — не API, man: «NOT API in any sense»)
launchctl print system/org.example.projectz.helper

# Отключено ли в launchd (persist через перезагрузки)
launchctl print-disabled system
launchctl enable  system/org.example.projectz.helper
launchctl disable system/org.example.projectz.helper
```

Что про них сказано: домен `system/[service-name]` — «anyone may read or query the system domain, but root privileges are required to make modifications»; `disable` значит «cannot be loaded … until it is once again enabled. This state persists across boots»; устаревшие `load/unload` заменены на `bootstrap/bootout/enable/disable` (`[M:launchctl]`). Для inline-запроса без root `launchctl print system/<несуществующий>` вернул `Bad request. Could not find service …` `[L:]` — т.е. отсутствие службы в домене различимо.

### 2.4. «Фоновые элементы» (macOS 13+) и признак «выключено пользователем»

Документировано Apple:

* Каждый LaunchAgent/LaunchDaemon/LoginItem имеет переключатель в System Settings → General → Login Items; для демона — запуск при старте системы. `[A:https://developer.apple.com/tutorials/data/documentation/servicemanagement/updating-helper-executables-from-earlier-versions-of-macos.json]`
* Имя в списке: с `AssociatedBundleIdentifiers` — имя приложения; иначе — организация из сертификата подписи; если приписать приложению нельзя и исполняемый файл неподписан — **имя исполняемого файла** из `Program`/`ProgramArguments`. Там же: у `AssociatedBundleIdentifiers` есть условия — **Team Identifier исполняемого файла должен совпадать с Team Identifier приложения**, и Launch Services должен знать приложение (`LSRegisterURL`). Следствие: **при ad-hoc подписи (ТЗ §13) привязка к приложению не сработает**, будет «Unknown Developer»/имя файла. Подтверждено наблюдением: у неподписанного стороннего демона в `sfltool dumpbtm` — `Identifier: Unknown Developer`, `Developer Name: (null)` `[L:]`.
* Проверка статуса legacy-plist: `SMAppService.statusForLegacyPlist(at: URL) -> SMAppService.Status`, macOS 13.0+; статусы `notRegistered`, `enabled`, `requiresApproval` («user must take action … before it can run»), `notFound`. Рекомендация Apple для приложений, которым нужны помощники: проверять статус при запуске, объяснить пользователю и вызвать `SMAppService.openSystemSettingsLoginItems()`. `[A:https://developer.apple.com/documentation/servicemanagement/smappservice]` (JSON-версия `https://developer.apple.com/tutorials/data/documentation/servicemanagement/smappservice.json`; статусы — `…/smappservice/status-swift.enum.json`; метод — `…/smappservice/statusforlegacyplist(at:).json`).
  Вызов из Rust-UI: крейт `objc2-service-management` 0.3.2 (`[crates.io]`, обновлён 2025-10-04) — наличие в нём `statusForLegacyPlist` **не проверялось** (§11); запасной путь — крошечный Swift/ObjC-хелпер или разбор `sfltool dumpbtm`.

Наблюдения на этой машине `[L:]` (`sfltool dumpbtm`, запускалось **без** sudo и вывело 792 строки):

* У каждого элемента есть поле `Disposition: [enabled|disabled, allowed|disallowed, visible, notified|not notified] (N)`; сопоставление числа и флагов по всем встреченным значениям: `11` = enabled+allowed+notified, `9` = enabled+disallowed+notified, `10` = disabled+allowed+notified, `2` = disabled+allowed+not notified, `3` = enabled+allowed+not notified. То есть биты: `0x1` enabled, `0x2` allowed, `0x8` notified (вывод из таблицы значений, не из документации).
* Тип нашего элемента: `Type: legacy daemon (0x10010)`, `URL: file:///Library/LaunchDaemons/<label>.plist`, `Executable Path`, `Parent Identifier` (запись разработчика).
* Есть legacy-демоны с `[enabled, disallowed, …] (9)`, и для такого демона `launchctl print system/<label>` отвечает «Could not find service» `[L:]` (пример: сторонний демон, исполняемый файл которого лежит в `/Applications`). При этом `launchctl print-disabled system` показывает только два служебных Apple-демона `[L:]` — то есть **выключение переключателем в «Фоновых элементах» не отражается в `launchctl print-disabled`**.

Практический способ для UI ТЗ §3 (в порядке надёжности): (1) `SMAppService.statusForLegacyPlist` (документирован); (2) `launchctl print system/<label>` — служба не найдена/не запущена при том, что plist установлен; (3) разбор `sfltool dumpbtm` по `Identifier == <Label>` и флагу `disallowed` — формат **не документирован**, может смениться, нужен как диагностика. Что именно меняет переключатель (`allowed` или `enabled`) — предположение по наблюдению, проверить на этапе 0 (§11).

Показ при установке (уведомление «Добавлены фоновые элементы») на странице Apple «Login Items» **не описано** `[A:https://support.apple.com/guide/mac-help/change-login-items-extensions-mac-mtusr003/13.0/mac/13.0]`; в `dumpbtm` есть признак `notified`, но связь не доказана (§11).

---

## 3. Служебный пользователь `_projectz`

### 3.1. Выбор UID/GID

* Для «ролевых» учёток `sysadminctl -h` говорит: имя должно начинаться с `_`, UID — в диапазоне **200–400** `[M:sysadminctl]` — у `sysadminctl` **нет man-страницы** (`man sysadminctl` пусто), источник — собственный вывод `sysadminctl -h` `[L:]`.
* Занятые на этой машине (macOS 13.7.8) `[L:]` `dscl . -list /Users UniqueID` и `/Groups PrimaryGroupID`: пользователи 200–291 и 441; группы 200–291, 395–400 (`com.apple.access_*`) и 441. Свободен диапазон **292–394** (и 401–440).
* Apple добавляет новые `_служебные` в следующие свободные номера (в 13.7 последний — 291 `_backgroundassets`), поэтому берите номер **с запасом от занятых** (например, из диапазона 340–394), одинаковый для UID и GID, проверяя свободу **по обоим** спискам; запись выбранных значений — в снимок установщика. Риск пересечения при будущем обновлении ОС — рассуждение, не факт (§11).
* Образец формата системной учётки (`[L:]` `dscl . -read /Users/_sntpd`): `NFSHomeDirectory: /var/empty`, `Password: *`, `PrimaryGroupID` = `UniqueID`, `RealName`, `RecordName`, `UserShell: /usr/bin/false`.

### 3.2. Создание (пример для установщика, выполняется от root; не запускалось)

Синтаксис `dscl` — из `[M:dscl]` (`-create record_path [key [val …]]`, `-delete path [key [val…]]`, `-read`, `-list`). Набор свойств — по образцу `_sntpd` выше.

```sh
UID_GID=345   # выбрать свободный (см. §3.1)

# Идемпотентная проверка: код 56 (eDSRecordNotFound) = записи нет  [L:] dscl . -read /Users/_projectz UniqueID
dscl . -read /Users/_projectz UniqueID >/dev/null 2>&1 && echo exists

# Группа
dscl . -create /Groups/_projectz
dscl . -create /Groups/_projectz PrimaryGroupID $UID_GID
dscl . -create /Groups/_projectz RealName "Project Z service group"

# Пользователь
dscl . -create /Users/_projectz
dscl . -create /Users/_projectz UniqueID $UID_GID
dscl . -create /Users/_projectz PrimaryGroupID $UID_GID
dscl . -create /Users/_projectz UserShell /usr/bin/false
dscl . -create /Users/_projectz NFSHomeDirectory /var/empty
dscl . -create /Users/_projectz RealName "Project Z service"
dscl . -create /Users/_projectz Password '*'
dscl . -create /Users/_projectz IsHidden 1        # скрыть из окна входа — см. §11
```

Альтернатива одной командой: `sysadminctl -addUser _projectz -UID 345 -GID 345 -shell /usr/bin/false -home /var/empty -roleAccount` (флаги из `sysadminctl -h` `[L:]`); требует интерактивных учётных данных администратора (`-adminUser/-adminPassword`) и группа с этим GID должна уже существовать (создавать `dscl`-ом, как выше). Не стоит класть пароль админа в командную строку (в `sysadminctl -h` это допускается, но пароль видят в `ps`); установщик, выполняющийся от root через osascript, использует `dscl`.

Удаление при деинсталляции (`[M:dscl]` `-delete`):

```sh
dscl . -delete /Users/_projectz
dscl . -delete /Groups/_projectz
# Альтернатива: sysadminctl -deleteUser _projectz  (запрашивает учётные данные админа, `[L:]` sysadminctl -h)
```

Перед удалением — остановить все процессы под этим пользователем (иначе остаются «сироты» с несуществующим uid) и убрать его файлы (владелец `_projectz` в `/Library/Application Support/ProjectZ/…`, если такие есть).

### 3.3. Запуск дочернего процесса под `_projectz` из Rust

Факты `[R:https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html]`:

* `CommandExt::uid(id)` и `gid(id)` — **стабильны** с 1.0.0; неудача `setuid/setgid` приводит к ошибке `spawn`.
* Если `groups` не заданы, `uid()` вызывает в потомке `setgroups(0, NULL)` — дополнительные группы root сбрасываются.
* `CommandExt::groups(&[u32])` — **nightly** (`feature = setgroups`, issue #90747) → в stable не использовать.
* `pre_exec` — `unsafe`, стабилен с 1.34.0, выполняется в потомке после `fork`; допустимы только async-signal-safe вызовы (никаких аллокаций, `std::env`, mutex).
* `process_group(0)` стабилен с 1.64.0; `setsid`/`chroot` — nightly.
* Tokio: `tokio::process::Command` имеет `uid`, `gid`, `pre_exec`, `process_group`, `kill_on_drop`, но **не имеет `groups`** `[crate:tokio-1.54.1]` `src/process/mod.rs`.

Пример (не компилировался):

```rust
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

pub struct ServiceUser { pub uid: u32, pub gid: u32 }

pub fn spawn_as_service_user(bin: &str, cfg: &str, u: &ServiceUser)
    -> std::io::Result<std::process::Child>
{
    let mut cmd = Command::new(bin);        // абсолютный путь внутри каталога root-владельца
    cmd.arg("run").arg("-config").arg(cfg)
        .env_clear()                         // не тащить окружение root
        .current_dir("/var/empty")
        .stdin(Stdio::null())
        .gid(u.gid)                          // gid и uid; std сам делает setgroups(0,NULL) → без доп. групп
        .uid(u.uid)
        .process_group(0);                   // отдельная группа процессов — удобно убивать целиком
    // Пока процесс не создал сокет, он ещё root-овый; pf смотрит uid *сокета* (создан уже после setuid)
    cmd.spawn()
}
```

Примечание: порядок «setgroups → setgid → setuid» внутри std — внутренняя деталь, в документации порядок не гарантируется; полагаться нужно на результат: проверить после старта, что `ps -o uid,gid,groups` показывает один gid (§11). Для pf важно, что сокет Xray создаётся **после** смены uid (`[M:pf.conf]`: учитывается uid на момент создания сокета).

---

## 4. Unix-сокет помощника и проверка клиента

### 4.1. Права

Факты `[M:unix]`: адрес — путь файловой системы **не длиннее 104 символов** (`sun_path[104]`, подтверждено `[SDK:usr/include/sys/un.h]`); файл сокета после закрытия **не удаляется** — нужен `unlink`; «normal filesystem access-control mechanisms are also applied … the destination of a connect(2) … must be writable». Ошибки `connect`: `EACCES` «Search permission is denied for a component of the path prefix» и «Write access to the named socket is denied» (`[M:connect]`, раздел ERRORS, проверено `man 2 connect`). `bind(2)`: `EEXIST` — файл уже есть, «unlink(2) it first» (`man 2 bind`).

Схема (согласуется с ТЗ §14: доступ «только пользователю, установившему приложение», плюс проверка uid):

| Объект | Владелец / режим | Зачем |
|---|---|---|
| каталог `/var/run/projectz/` (или `/Library/Application Support/ProjectZ/run/`) | `root:wheel`, `0755` (прохождение разрешено всем, запись — только root) | никто не подменит сокет; путь короткий (<104) |
| файл сокета `helper.sock` | владелец — uid пользователя-установщика (записывается при установке), `0600` | `connect` разрешён (нужна запись) только владельцу и root |

(Выбор каталога `/var/run` и что он очищается при загрузке — не проверялся, §11; помощник при каждом старте обязан сам создать каталог и сделать `unlink` старого файла.)

Порядок создания в коде: `umask(0o177)` → `unlink(path)` → `bind` → `chown(path, installer_uid, gid)` → `chmod 0600`. Окно между `bind` и `chown` безопасно, если каталог закрыт (`0700 root`) до готовности; если каталог `0755`, лучше `bind` во временное имя в закрытом подкаталоге и `rename`.

### 4.2. Проверка uid подключившегося

* `[M:getpeereid]`: `getpeereid(s, &euid, &egid)` возвращает **эффективные** uid/gid пира на момент `connect` (для слушающей стороны) — «reliable; no way for either side to influence the credentials returned to its peer except by calling … connect/listen under different effective credentials». На macOS это реализовано через `LOCAL_PEERCRED` (`[M:unix]`: struct `xucred` из `sys/ucred.h`).
* `[SDK:usr/include/sys/un.h]`: `LOCAL_PEERCRED 0x001`, `LOCAL_PEERPID 0x002`, `LOCAL_PEEREPID 0x003`, `LOCAL_PEERUUID 0x004`, `LOCAL_PEEREUUID 0x005`, `LOCAL_PEERTOKEN 0x006` (audit token), `SOL_LOCAL 0`.

Крейты (по исходникам):

| Крейт | Что даёт на macOS |
|---|---|
| **tokio 1.54.1** | `UnixStream::peer_cred() -> io::Result<UCred>`; `UCred { uid(), gid(), pid() -> Option<pid_t> }`. На Apple реализовано вызовом `getpeereid` + `getsockopt(SOL_LOCAL, LOCAL_PEEREPID)`; `pid()` на Apple возвращает `Some` `[crate:tokio-1.54.1]` `src/net/unix/ucred.rs` (`impl_macos`). Это самый простой путь. |
| **nix 0.32.0** | `nix::unistd::getpeereid(fd) -> Result<(Uid, Gid)>` (`cfg(bsd)`, нужны фичи `socket` и `user`); `sys::socket::sockopt::{LocalPeerCred, LocalPeerPid, LocalPeerToken}` (`cfg(apple_targets)`, нужна фича `socket`; `LocalPeerCred` → `XuCred { version(), uid(), groups() }`; `LocalPeerToken` → `audit_token_t`) `[crate:nix-0.32.0]` `src/unistd.rs`, `src/sys/socket/sockopt.rs`, `src/sys/socket/mod.rs`. |
| **rustix 1.1.6** | `net::sockopt::socket_peercred` — только `cfg(linux_kernel)` (`SO_PEERCRED`); **на macOS недоступен** `[crate:rustix-1.1.6]` `src/net/sockopt.rs`. |
| **std** | `UnixStream::peer_cred` и `UCred` — `#[unstable(feature = "peer_credentials_unix_socket")]` `[R:https://raw.githubusercontent.com/rust-lang/rust/1.100.0/library/std/src/os/unix/net/ucred.rs]` → в stable недоступны. |
| **interprocess 2.4.8** | кроссплатформенный `local_socket` (Unix-сокеты и именованные каналы Windows) с типом `PeerCreds` для аутентификации `[crate:interprocess-2.4.8]` `src/local_socket.rs` (док-коммент: «authentication (PeerCreds)»). Подходит для единого кода macOS/Windows на этапе 2; детали `PeerCreds` на macOS в этой справке не разбирались. |

Пример (tokio; не компилировался):

```rust
use tokio::net::{UnixListener, UnixStream};

async fn serve(listener: UnixListener, allowed_uid: u32) -> std::io::Result<()> {
    loop {
        let (stream, _addr) = listener.accept().await?;
        match authorize(&stream, allowed_uid) {
            Ok(pid) => { tokio::spawn(handle(stream, pid)); }
            Err(e)  => { tracing::warn!(error = %e, "отказ в подключении"); /* stream закрывается */ }
        }
    }
}

fn authorize(s: &UnixStream, allowed_uid: u32) -> Result<Option<i32>, &'static str> {
    let cred = s.peer_cred().map_err(|_| "не удалось получить учётные данные")?;
    // root (0) допускаем только для служебных команд CLI «сбросить всё» (ТЗ §9); пользователь — установщик
    if cred.uid() == allowed_uid || cred.uid() == 0 { Ok(cred.pid()) } else { Err("чужой uid") }
}
```

Через nix (если нужен `LOCAL_PEERTOKEN` для проверки подписи клиента):

```rust
use nix::sys::socket::{getsockopt, sockopt::LocalPeerToken};
let tok = getsockopt(&stream, LocalPeerToken)?;   // audit_token_t; фичи nix: "socket"
```

Токен в протоколе — дополнение (ТЗ §14), а не единственная защита. Что PID можно переиспользовать, поэтому проверять по PID небезопасно, а по audit-токену надёжнее — общеизвестное соображение, источника в этой справке нет (§11).

---

## 5. Системные настройки: часовой пояс, DNS, интерфейсы, события

### 5.1. Часовой пояс

| Что | Команда / место | Источник |
|---|---|---|
| прочитать пояс без root | `readlink /etc/localtime` → `/var/db/timezone/zoneinfo/Europe/Moscow` (т.е. IANA-имя — хвост пути) | `[L:]` (`/etc/localtime` — символическая ссылка) |
| список допустимых имён | `systemsetup -listtimezones`; в каталоге `/var/db/timezone/zoneinfo` лежат IANA-зоны | `[M:systemsetup]`; `[L:]` `ls /var/db/timezone` |
| прочитать пояс (root) | `systemsetup -gettimezone` | `[M:systemsetup]`; **без root отказ**: «You need administrator access to run this tool» `[L:]` (даже для пользователя из группы admin) |
| установить пояс (root) | `systemsetup -settimezone America/New_York` (в man — пример `US/Pacific`); принимает ли IANA-имена так же, как алиасы — проверить | `[M:systemsetup]` |
| флаг «устанавливать пояс автоматически» | `/Library/Preferences/com.apple.timezone.auto.plist`, ключ `Active` (целое 0/1), владелец `root:wheel`, `-rw-r--r--` | `[L:]` `defaults read /Library/Preferences/com.apple.timezone.auto` → `{ Active = 1; }`; `plutil -p` → `"Active" => 1` |
| прочитать флаг | `defaults read /Library/Preferences/com.apple.timezone.auto Active` | `[L:]` |
| «время по сети» (это **не** пояс) | `systemsetup -getusingnetworktime`, `sysadminctl -automaticTime status`/`on|off` | `[M:systemsetup]`; `[L:]` `sysadminctl -automaticTime status` → «Automatic time enabled» |

Что именно нужно сделать при переключении `Active` (одной записью `defaults write` от root или ещё и уведомить демона) и подействует ли это сразу — **не подтверждено** (§11). Поэтому в снимок (ТЗ §9) писать и IANA-имя из `readlink /etc/localtime`, и значение `Active`, а при откате ставить сначала флаг, потом пояс (или наоборот — выбрать после проверки на этапе 0).

### 5.2. Сброс кэша DNS

* `dscacheutil -flushcache` — `[M:dscacheutil]` («Flushes the entire cache. This should only be used in extreme cases.»; расположение: `/usr/bin/dscacheutil` `[L:]`).
* `killall -HUP mDNSResponder` — в ТЗ §9; **man-страница `mDNSResponder` на этой машине отсутствует**, подтвердить нечем (§11). Выполняет root-помощник после снятия TUN (ТЗ §9).

### 5.3. Активный физический интерфейс и его DNS

Наблюдения на этой машине при активном стороннем VPN `[L:]`:

| Команда | Что показала |
|---|---|
| `route -n get default` | `interface: utun8` — то есть маршрут по умолчанию уже указывает на туннель другого VPN; **физический интерфейс этой командой не определить** |
| `route -n get -ifscope en0 default` | `gateway: 192.168.0.1, interface: en0, flags …IFSCOPE` — маршрут по умолчанию, привязанный к интерфейсу (`-ifscope` описан в `[M:route]`) |
| `netstat -rn -f inet` | две строки `default`: `link#18 … utun8` и `192.168.0.1 … en0` (флаг `I` = ifscope) |
| `scutil --nwi` | список «Network interfaces: utun8 en0», у каждого флаги `(IPv4,DNS)`; для VPN-интерфейса — поле `VPN server` |
| `scutil --dns` | в секции «DNS configuration (for scoped queries)»: `nameserver[0] : 192.168.0.1`, `if_index : 4 (en0)`, `flags : Scoped` — **DNS физического интерфейса виден именно здесь**; в основном списке при активном VPN у `resolver #1` серверов может не быть |
| `networksetup -listallhardwareports` | пары «Hardware Port ↔ Device»: `Wi-Fi → en0`, `Thunderbolt Bridge → bridge0` … — отсеивает `utunN`, `awdl0`, `llw0` |
| `ifconfig -l` | `utun0 … utun8` присутствуют **всегда** (системные `utun` для iCloud/Handoff и др.) |

Из этого — **уточнение к ТЗ §4:** определять активный физический интерфейс «по маршруту по умолчанию» нельзя, пока в системе работает другой VPN (и после запуска собственного TUN); надёжнее брать данные из динамического хранилища SystemConfiguration (читаются без root через `scutil` / SCDynamicStore):

```
State:/Network/Global/IPv4                   → PrimaryInterface, PrimaryService, Router   (при VPN → utunN)
State:/Network/Service/<id>/IPv4             → InterfaceName, Router, Addresses, SubnetMasks, NetworkSignature
State:/Network/Service/<id>/DNS              → ServerAddresses (DNS именно этого сервиса)
State:/Network/Interface/<if>/Link           → есть ли линк
```

`[L:]` `scutil` → `show State:/Network/Global/IPv4`, `list State:/Network/Service/.*/DNS`, `show State:/Network/Service/<id>/DNS` (получен список DNS `192.168.0.1`). Алгоритм помощника: перебрать `State:/Network/Service/*/IPv4`, оставить сервисы, у которых `InterfaceName` — аппаратный порт (из `SCNetworkInterfaceCopyAll`/`networksetup`), выбрать по порядку сервисов (`Setup:/Network/Global/IPv4` → `ServiceOrder` `[L:]`), взять `Router` и `ServerAddresses` этого сервиса.

### 5.4. Подписка на смену сети и сон/пробуждение из Rust

**Сеть: `system-configuration` 0.8.0** (зависит от `system-configuration-sys` 0.6.0 и `core-foundation` 0.10.1; `[crates.io]`; MSRV 1.64 — `[crate:system-configuration-0.8.0]`). API (`src/dynamic_store.rs`, пример `examples/watch_dns.rs`):

* `SCDynamicStoreBuilder::new(name).callback_context(SCDynamicStoreCallBackContext { callout: fn, info }).build() -> Option<SCDynamicStore>`;
* **`callout` — обычная `fn(SCDynamicStore, CFArray<CFString>, &mut T)`, а не замыкание** — состояние передавайте через `info` (например, `tokio::sync::mpsc::UnboundedSender`);
* `store.set_notification_keys(&keys, &patterns) -> bool` — шаблоны — регулярные выражения (в примере `"(State|Setup):/Network/Service/.*/DNS"`);
* `store.create_run_loop_source()`, затем `CFRunLoop::get_current().add_source(&src, kCFRunLoopCommonModes)` и `CFRunLoop::run_current()` — **нужен отдельный поток с CFRunLoop**;
* также `get(key)`, `get_keys(pattern)`, `get_proxies()`.

```rust
use core_foundation::{array::CFArray, runloop::{kCFRunLoopCommonModes, CFRunLoop}, string::CFString};
use system_configuration::dynamic_store::{SCDynamicStore, SCDynamicStoreBuilder, SCDynamicStoreCallBackContext};
use tokio::sync::mpsc::UnboundedSender;

fn on_change(_s: SCDynamicStore, keys: CFArray<CFString>, tx: &mut UnboundedSender<Vec<String>>) {
    let _ = tx.send(keys.iter().map(|k| k.to_string()).collect());
}

pub fn spawn_net_watcher(tx: UnboundedSender<Vec<String>>) {
    std::thread::spawn(move || {
        let store = SCDynamicStoreBuilder::new("projectz-helper")
            .callback_context(SCDynamicStoreCallBackContext { callout: on_change, info: tx })
            .build().expect("SCDynamicStore");
        let keys: CFArray<CFString> = CFArray::from_CFTypes(&[]);
        let patterns = CFArray::from_CFTypes(&[
            CFString::from("State:/Network/Global/IPv4"),
            CFString::from("State:/Network/Global/IPv6"),
            CFString::from("State:/Network/Interface/.*/Link"),
            CFString::from("State:/Network/Service/.*/IPv4"),
            CFString::from("State:/Network/Service/.*/DNS"),
        ]);
        assert!(store.set_notification_keys(&keys, &patterns));
        let src = store.create_run_loop_source().expect("run loop source");
        CFRunLoop::get_current().add_source(&src, unsafe { kCFRunLoopCommonModes });
        CFRunLoop::run_current();
    });
}
```

Нюанс: собственный TUN sing-box тоже порождает события (`utunN`, `Global/IPv4`) — фильтруйте события по интерфейсу (своё имя `utunN` игнорировать), иначе получите цикл «перепривязка → событие → перепривязка».

**Сон/пробуждение: IOKit.** В SDK `[SDK:System/Library/Frameworks/IOKit.framework/Headers/pwr_mgt/IOPMLib.h]`:

* `io_connect_t IORegisterForSystemPower(void *refcon, IONotificationPortRef *thePortRef, IOServiceInterestCallback callback, io_object_t *notifier)`; освобождение: `IODeregisterForSystemPower(&notifier)`, затем `IOServiceClose(connect)`; порт уничтожить отдельно.
* `IOServiceInterestCallback(void *refcon, io_service_t service, uint32_t messageType, void *messageArgument)`.
* **Обязательно подтверждать** `kIOMessageSystemWillSleep` и `kIOMessageCanSystemSleep` вызовом `IOAllowPowerChange(connect, (intptr_t)messageArgument)`; без подтверждения сон продолжится после 30 секунд ожидания; `IOCancelPowerChange` — только для `CanSystemSleep` (вето).
* Сообщения (`[SDK:…/IOKit.framework/Headers/IOMessage.h]`): `kIOMessageCanSystemSleep = iokit_common_msg(0x270)`, `kIOMessageSystemWillSleep = …(0x280)`, `kIOMessageSystemWillNotSleep = …(0x290)`, `kIOMessageSystemHasPoweredOn = …(0x300)`, `kIOMessageSystemWillPowerOn = …(0x320)`. Числовые значения: `sys_iokit = err_system(0x38)`, `sub_iokit_common = err_sub(0)` (`IOReturn.h`) → `iokit_common_msg(x) = 0xE0000000 | x` (вычисление `0x38 << 26 = 0xE0000000`, мой расчёт; сверьте `#define` в SDK перед использованием).
* Источник событий для run loop: `IONotificationPortGetRunLoopSource(port)` — **есть** в крейте `io-kit-sys` 0.5.0 `[crate:io-kit-sys-0.5.0]` `src/lib.rs`, как и `IOServiceClose`; а вот **`IORegisterForSystemPower`, `IOAllowPowerChange` в `io-kit-sys` 0.5.0 отсутствуют** (поиск по исходникам пуст; модуль `pwr_mgt/pm.rs` содержит лишь константы `kIOPMAssertions…`). Значит: объявить `extern "C"` самим с `#[link(name = "IOKit", kind = "framework")]`.

Альтернативы (если не хочется FFI): события сна обычно сопровождаются сменой состояния сети (`State:/Network/Interface/*/Link`) и тайм-джампом таймера — как *дополнительный* признак пробуждения (рассуждение; не из документации) — полагаться только на них нельзя, т.к. ТЗ §4.1 требует блок именно при сне/пробуждении.

---

## 6. Обнаружение конфликтов

### 6.1. Другой активный VPN/TUN

Признаки (с наблюдений на этой машине `[L:]`, где активен сторонний VPN на `utun8`):

1. `State:/Network/Global/IPv4` → `PrimaryInterface : utun8` (и `scutil --nwi` показывает у этого интерфейса `VPN server`); при отсутствии VPN первичным будет `en0`. Проверять нужно **до** запуска своего TUN.
2. `netstat -rn -f inet` — вторая строка `default` на `utunN` (маршрут без шлюза `link#N`) рядом с `default` на физический интерфейс.
3. `scutil --nc list` — перечень VPN-конфигураций с состоянием `(Connected)`/`(Disconnected)` и типом (`[VPN:<bundle id>]`) `[L:]`, `[M:scutil]` (`--nc` — «commands for monitoring and interacting with VPN connections»). Работает для VPN, созданных через NetworkExtension/системные сервисы.
4. Одно лишь наличие `utunN` **не признак**: `utun0…utun7` существуют всегда `[L:]`.
5. Системный прокси: у стороннего клиента может стоять PAC (`scutil --proxy` → `ProxyAutoConfigEnable`, `ProxyAutoConfigURLString` `[L:]`; формат — `[M:scutil]` `--proxy`) — это конфликт с ТЗ §9 («системный прокси не меняем», но чужой нас может сломать).

### 6.2. iCloud Private Relay

* Что подтверждено Apple: чтобы **сеть** заблокировала Private Relay, нужно отвечать на запросы `mask.icloud.com` и `mask-h2.icloud.com` «NOERROR без ответа» или NXDOMAIN; на устройстве пользователю показывается предупреждение отключить Private Relay для этой сети или сменить сеть. `[A:https://developer.apple.com/support/prepare-your-network-for-icloud-private-relay/]`. Охват: трафик Safari, DNS-запросы и незашифрованный HTTP приложений (там же).
* Как **определить на самом Mac**, что Private Relay включён, — **подтвердить не удалось**. Найдено лишь: в `~/Library/Preferences/com.apple.networkserviceproxy.plist` есть ключи `NSPConfiguration`, `NSPServiceStatusManagerInfo` (внутри — бинарный `NSKeyedArchiver`-архив), процесс `networkserviceproxy` запущен `[L:]`; формат не документирован. Из страниц поддержки Apple (`support.apple.com/en-us/102602`, «Use iCloud Private Relay» в mac-help) выяснить поведение при включённом VPN **не удалось** — страницы это не описывают. Поэтому в UI честно: «не удалось определить автоматически — проверьте System Settings → Apple ID → iCloud → Private Relay» (путь меню — предположение, §11).

---

## 7. Версии (crates.io, 2026-11-15) и заметки по API

Rust: **1.100.0 (2026-11-09)**, канал stable от 2026-11-12 `[R:https://static.rust-lang.org/dist/channel-rust-stable.toml]` (`[pkg.rust] version = "1.100.0 (0ef1bc386 2026-11-09)"`).
Edition: **2024** (стабильна с 1.85). Подтверждение косвенное: свежие крейты `clap 4.6.7`, `sha2 0.11.1`, `rand 0.10.3`, `time 0.3.55` объявляют `edition = "2024"` и `rust-version = 1.85…1.88` `[crate:…/Cargo.toml]`; страница Edition Guide сама список редакций не дала (§11). Новой редакции после 2024 не обнаружено.
Цели: `x86_64-apple-darwin` — **Tier 2 with host tools** (macOS 10.12+ по умолчанию), `aarch64-apple-darwin` — **Tier 1 with host tools** (macOS 11.0+) `[R:https://doc.rust-lang.org/rustc/platform-support.html]`. Для нижней границы **macOS 13** (ТЗ §2) задать при сборке `MACOSX_DEPLOYMENT_TARGET=13.0` (умолчание компилятора — 10.12/11.0; переменная — стандартный механизм Apple-линкера, в Rust-документации здесь не проверялась, §11).

Установка (`[R:https://rust-lang.github.io/rustup/installation/other.html]`):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# опции rustup-init: sh -s -- --no-modify-path  /  --default-toolchain stable
rustup target add x86_64-apple-darwin aarch64-apple-darwin   # команда rustup target add — страница её не описывает (§11)
rustup default stable
```

Pин toolchain в репозитории — файл `rust-toolchain.toml` (`channel = "1.100.0"`, `targets = [...]`) — рекомендация; документация по нему здесь не сверялась (§11).

### 7.1. Таблица версий

| Крейт | Стабильная | MSRV | Замечание |
|---|---|---|---|
| tokio | 1.54.1 | 1.85 | edition 2021 |
| serde | 1.0.229 | — | с фичей `derive` |
| serde_json | 1.0.154 | — | |
| thiserror | 2.0.21 | 1.77 | ветка 2.x |
| anyhow | 1.0.104 | — | |
| tracing | 0.1.44 | — | |
| tracing-subscriber | 0.3.23 | 1.65 | фичи `env-filter`, `fmt`, `json` — опциональны |
| nix | 0.32.0 | 1.69 | **docs.rs не собрал 0.32.0** (последняя собранная — 0.31.3); все модули за фичами |
| rustix | 1.1.6 | 1.65 | |
| libc | 0.2.191 | — | |
| sha2 | 0.11.1 | 1.88 | edition 2024; `digest` 0.11; MSRV поднимается в патч-релизах |
| hex | 0.4.3 | — | последний релиз 2021-03-03 |
| reqwest | 0.13.5 | 1.85 | rustls по умолчанию |
| url | 2.5.8 | — | |
| ipnet | 2.12.2 | — | edition 2018 |
| rand | 0.10.3 | 1.85 | **новый API** (§7.2) |
| tempfile | 3.27.0 | 1.63 | |
| time | 0.3.55 | 1.88 | edition 2024 |
| chrono | 0.4.45 | — | альтернатива `time` |
| uuid | 1.29.2 | **1.89** | фича `v4` |
| clap | 4.6.7 | 1.85 | edition 2024; фича `derive` |
| hickory-resolver | 0.26.3 | 1.88 | **API 0.26** (§7.2) |
| system-configuration | 0.8.0 | 1.64 | + `system-configuration-sys` 0.6.0 |
| core-foundation | 0.10.1 | — | |
| io-kit-sys | 0.5.0 | 1.65 | нет `IORegisterForSystemPower` |
| windows | 0.62.2 | — | этап 2; конкретные фичи (`Win32_…`) подбирать по нужным API |
| interprocess | 2.4.8 | 1.75 | фичи `tokio` и др. — см. Cargo.toml крейта |
| objc2-service-management | 0.3.2 | — | для `SMAppService` из UI (§2.4) |

Источники столбцов: версии/даты — `[crates.io]` (`max_stable_version`); MSRV и edition — `Cargo.toml` скачанных крейтов `[crate:…]`; для крейтов без значения MSRV не проверялся.

### 7.2. Заметки по API (только то, что подтверждено исходниками)

* **tokio 1.54.1**: `UnixListener`/`UnixStream`; `UnixStream::peer_cred()`; `tokio::process::Command` (`uid`, `gid`, `pre_exec`, `process_group`, `kill_on_drop`, нет `groups`). Фичи включаются явно (`rt-multi-thread`, `macros`, `net`, `process`, `signal`, `sync`, `time`, `io-util`, `fs`).
* **nix 0.32.0**: `getpeereid` и `sockopt::LocalPeer*` см. §4.2; `FcntlArg::F_FULLFSYNC` (`cfg(apple_targets)`, `src/fcntl.rs`); `unistd::{setuid,setgid,setgroups,initgroups,fsync}` — фичи `user`/`fs`; сигнатуры из-за сбоя docs.rs 0.32.0 брались из исходников.
* **rustix 1.1.6**: `fs::fcntl_fullfsync(fd)` — есть для Apple (`src/fs/fcntl_apple.rs`); `net::sockopt::socket_peercred` — только Linux. Подходит как замена `libc::fcntl` без `unsafe`.
* **rand 0.10.3**: `rand::rng()` (нужна фича `thread_rng`, входит в умолчания `std`), трейты `rand::Rng` (= `rand_core::Rng`) и **`rand::RngExt`** с методами `random()`, `random_range(range)`, `fill(&mut [T])` (`src/lib.rs`, `src/rng.rs`). Старые `gen`, `gen_range`, `thread_rng()` из 0.8/0.9 использовать нельзя. Для секретов (токен IPC, пароль socks Xray) — `rand::rngs::SysRng` (фича `sys_rng`) надёжнее ручных построений; пример использования API не проверялся.
* **sha2 0.11.1**: `use sha2::{Sha256, Digest}; Sha256::digest(data)`; `Sha256::new()` + `update` + `finalize()` — README. Формат результата в 0.11 — новый тип (`digest` 0.11, «Replace type aliases with newtypes»), поэтому `hex::encode(&hash[..])`/`{:x}` **не проверялись** — сверить при первом `cargo build` (§11).
* **reqwest 0.13.5**: `default = [default-tls (= rustls), charset, http2, system-proxy]`; `rustls` = `aws-lc-rs` + `rustls-platform-verifier` (системные корневые сертификаты); **`json`, `query`, `form`, `stream`, `gzip`, `socks`, `blocking` — отдельные фичи**. `ClientBuilder::no_proxy()` — обязательно для помощника, потому что `system-proxy` включён по умолчанию и подхватит чужой PAC/прокси (`[L:]` на этой машине системный PAC настроен). `ClientBuilder::redirect(Policy::custom(..))` — для правила «редиректы только на https и не на частные адреса» (ТЗ §5.1); `ClientBuilder::interface("en0")` доступен на macOS (`cfg(target_os = "macos")`), `local_address(ip)`, `resolve(domain, addr)` — подмена DNS на заранее найденный IP; `dns_resolver(...)` — свой резолвер `[crate:reqwest-0.13.5]` `src/async_impl/client.rs`, `Cargo.toml`. Что `interface()` на macOS реализовано через `IP_BOUND_IF` — не проверено (§11).
* **hickory-resolver 0.26.3**: 100% собственный резолвер, системный (libc) резолвер **не использует** (`src/lib.rs` док-коммент). Создание: `Resolver::builder_tokio()?` (читает `/etc/resolv.conf`) либо `Resolver::builder_with_config(ResolverConfig::udp_and_tcp(&GOOGLE), TokioRuntimeProvider::default()).build()?`; `resolver.lookup_ip("name.")`. **Для macOS `builder_tokio()` не годится**: сам файл `/etc/resolv.conf` на macOS сообщает «not consulted for DNS hostname resolution» `[L:]` (шапка `/etc/resolv.conf`) — серверы нужно брать из SCDynamicStore (§5.3) и задавать явно через `NameServerConfig::udp_and_tcp(ip)` (поля: `ip`, `trust_negative_responses`, `connections: Vec<ConnectionConfig>`). У `ConnectionConfig` есть **`bind_addr: Option<SocketAddr>`** — привязка к локальному IP физического интерфейса. Но привязка к *адресу* не равна привязке к *интерфейсу*: при перехваченной таблице маршрутов пакет с адресом en0 всё равно может уйти по маршруту через TUN — на macOS для надёжности нужен `IP_BOUND_IF` (отдельный сокет-опция), которого в этом крейте не нашлось → при необходимости резолвер-процесс писать на `std::net::UdpSocket` + `setsockopt(IP_BOUND_IF)` через `libc`/`nix` или использовать `route -ifscope` (§11). DoT/DoH — фичи `tls-*`/`https-*` с выбором провайдера криптографии (`aws-lc-rs` или `ring`).
* **tempfile 3.27.0**: `NamedTempFile::new_in(dir)` и `persist(path)` (= `rename`), `persist_noclobber`; **`fsync` не вызывает** — для атомарной записи (§8) нужен собственный `sync_all`.
* **tracing-subscriber 0.3.23**: фичи по умолчанию — `fmt` и др. (см. `Cargo.toml`); `env-filter`, `json` — отдельные. Ротация файлов — отдельный крейт (`tracing-appender`; в этой справке не проверялся).
* **uuid 1.29.2**: MSRV 1.89 (самый высокий среди перечисленных) — Rust 1.100 удовлетворяет.
* **ipnet 2.12.2**, **url 2.5.8**, **hex 0.4.3**, **serde/serde_json/thiserror/anyhow/clap/time/chrono/windows**: версии подтверждены, API в этой справке не разбирался.
* **interprocess 2.4.8**: см. §4.2. Для macOS-этапа достаточно tokio `UnixListener`; interprocess уместен, если на этапе 2 хотите общий код для Unix-сокета и named pipe.
* **system-configuration 0.8.0 / io-kit-sys 0.5.0**: см. §5.4.

Рекомендация по совместимости: зафиксировать версии `Cargo.lock`, MSRV workspace = 1.89 (из-за `uuid`), т.к. часть крейтов повышает MSRV в патч-релизах (CHANGELOG `sha2`: «allow MSRV bumps in patch releases»).

---

## 8. Атомарная запись файла на macOS

Факты:

* `fsync()` на macOS переносит данные на устройство, но **накопитель может держать их в кэше**; для строгого порядка есть `fcntl(F_FULLFSYNC)` — «Does the same thing as fsync(2) then asks the drive to flush all buffered data to the permanent storage device» (реализовано на HFS, FAT, UDF и APFS), «may take quite a while» `[M:fsync]`, `[M:fcntl]`.
* **`std::fs::File::sync_all()` на Apple-платформах уже вызывает `fcntl(F_FULLFSYNC)`**, а на остальных — `fsync` `[R:https://raw.githubusercontent.com/rust-lang/rust/1.100.0/library/std/src/sys/fs/unix.rs]` (функция `fsync`, `#[cfg(target_vendor = "apple")] … libc::F_FULLFSYNC`; `datasync` — тоже). Поэтому отдельный `libc::fcntl` не нужен; `rustix::fs::fcntl_fullfsync` и `nix` `FcntlArg::F_FULLFSYNC` — равнозначные альтернативы.
* `rename(2)` в пределах одной файловой системы заменяет цель; в man есть и обмен через флаги (`[M:rename]` упоминает «atomically swapped»). Каталог-источник и каталог-цель должны быть на одном томе → временный файл создаём **в том же каталоге**.
* Надёжность записи каталога (`fsync` каталога после `rename`) — стандартная практика; **что `F_FULLFSYNC` на дескрипторе каталога APFS работает без ошибки, в man не сказано** → код терпимо относится к `ENOTSUP`/`EINVAL`/`ENOTTY` для каталога (§11).

Пример (не компилировался):

```rust
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Атомарная запись: tmp в том же каталоге → write → sync_all (F_FULLFSYNC на macOS) → rename → sync каталога.
pub fn atomic_write(path: &Path, data: &[u8], mode: u32) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "нет каталога"))?;
    let name = path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "нет имени"))?;
    let tmp = dir.join(format!(".{}.tmp.{}", name.to_string_lossy(), std::process::id()));

    let result = (|| -> io::Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)          // O_EXCL: не затираем чужой tmp
            .mode(mode)                // например 0o600 для снимка с секретами
            .open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;                 // macOS: fcntl(F_FULLFSYNC)
        drop(f);
        fs::rename(&tmp, path)?;       // атомарная подмена
        // fsync каталога, чтобы переименование пережило сбой питания
        match File::open(dir).and_then(|d| d.sync_all()) {
            Ok(()) => Ok(()),
            Err(e) if matches!(e.raw_os_error(),
                Some(libc::ENOTSUP) | Some(libc::EINVAL) | Some(libc::ENOTTY)) => Ok(()),
            Err(e) => Err(e),
        }
    })();

    if result.is_err() { let _ = fs::remove_file(&tmp); }
    result
}
```

Замечание: при `sync_all()` на каталоге ошибка `ENOTSUP`/`EINVAL` теоретически возможна для томов без `F_FULLFSYNC` (список errno подобран автором из-за отсутствия в man; сверить на этапе 0).

---

## 9. Что меняет/уточняет ТЗ (главные находки)

1. **§7 pf, порядок правил:** правило «резолвер → DNS-серверы» должно стоять выше блока «DNS на LAN», потому что DNS-сервер сети обычно — роутер в LAN (на этой машине `192.168.0.1`). Иначе bootstrap-DNS не работает (§1.3, п. 2).
2. **§7 pf, utun:** «pass всё на utun» не нужно и вредно планировать: имя TUN заранее неизвестно (sing-box выбирает сам). Блокировать надо только физические интерфейсы (список от помощника); трафик на `utunN` по умолчанию проходит (§1.1).
3. **§7 pf, anchor внутри `com.apple/*`:** по man — работает (главное правило `anchor "com.apple/*"` без `quick`, вычисляет прямых потомков по алфавиту); все наши правила `quick`; `user` задавать **числом**; на практике проверить на этапе 0 (§1.2).
4. **§4 «следит за сменой активного интерфейса»:** `route -n get default` недостоверен при стороннем VPN/нашем TUN — брать данные из SCDynamicStore (`State:/Network/Service/<id>/IPv4|DNS`) и аппаратных портов; DNS физической сети виден в `scutil --dns` в секции scoped (§5.3).
5. **§3/§13 «Фоновые элементы»:** при ad-hoc подписи привязка к приложению (`AssociatedBundleIdentifiers`) не сработает — требуется совпадение Team Identifier; в списке будет «Unknown Developer»/имя исполняемого файла. Состояние «выключено пользователем» **не видно** в `launchctl print-disabled`; документированный способ — `SMAppService.statusForLegacyPlist(at:)` (macOS 13+) из UI-процесса; запасной — `sfltool dumpbtm` (читается без sudo на этой машине, формат не документирован) (§2.4).
6. **§3 дочерние процессы:** в stable `CommandExt::groups` нет; `uid()` сам делает `setgroups(0, NULL)` — этого достаточно, но результат проверять (§3.3).
7. **§4 bootstrap-DNS:** `hickory-resolver::builder_tokio()` читает `/etc/resolv.conf`, а macOS его для резолва не использует → серверы задавать явно (из SCDynamicStore). Привязка к интерфейсу: у hickory только `bind_addr` (адрес), а `IP_BOUND_IF` потребует собственного сокета (§7.2).
8. **§5.1 «загрузка подписки»:** reqwest 0.13 включает `system-proxy` по умолчанию → в помощнике использовать `no_proxy()` и `default-features = false` (иначе подхватится чужой PAC; на этой машине он есть). Фичи `json` и др. — явные (§7.2).
9. **§9/§4.1 снимок:** `File::sync_all()` на macOS уже `F_FULLFSYNC` (проверено по исходнику std 1.100.0); `tempfile::persist` fsync не делает; пример атомарной записи — §8.
10. **§10.1 часовой пояс:** флаг лежит в `/Library/Preferences/com.apple.timezone.auto.plist` ключ `Active` (прочитано: 1); пояс без root читается через `readlink /etc/localtime`; `systemsetup` без root отказывает даже для admin (§5.1).
11. **Rust:** стабильная 1.100.0, edition 2024; `x86_64-apple-darwin` — **Tier 2** (не Tier 1) — основное тестирование ТЗ на Intel идёт на менее приоритетной цели; `uuid 1.29` требует Rust 1.89 → MSRV workspace ≥ 1.89 (§7).
12. **§4.1 сон/пробуждение:** готового безопасного крейта нет; `io-kit-sys` не содержит `IORegisterForSystemPower` — нужен собственный `extern "C"` + обязательный `IOAllowPowerChange` при `WillSleep`/`CanSystemSleep` (иначе сон задерживается на 30 с) (§5.4).
13. **§4.1 «Конфликты»:** признак «другой VPN» — первичный интерфейс `utunN` + `scutil --nc list`; наличие `utunN` само по себе ничего не значит. Определение Private Relay на Mac не подтверждено (§6).

---

## 10. Источники (полный перечень)

Man-страницы (macOS 13.7): `pfctl`, `pf.conf`, `launchd.plist`, `launchctl`, `dscl`, `systemsetup`, `scutil`, `route`, `tcpdump`, `unix`, `getpeereid`, `fcntl`, `fsync`, `rename`, `dscacheutil`, `connect(2)`, `bind(2)`. У `sysadminctl` man-страницы нет — использован `sysadminctl -h`.
Файлы: `/etc/pf.conf`, `/etc/pf.anchors/com.apple`.
SDK: `/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/include/sys/un.h`, `System/Library/Frameworks/IOKit.framework/Headers/{pwr_mgt/IOPMLib.h, IOMessage.h, IOReturn.h, IOKitLib.h}`.
Apple: `https://developer.apple.com/documentation/servicemanagement/smappservice` (+ JSON-версии `developer.apple.com/tutorials/data/documentation/servicemanagement/…`), `https://developer.apple.com/support/prepare-your-network-for-icloud-private-relay/`, `https://support.apple.com/guide/mac-help/change-login-items-extensions-mac-mtusr003/13.0/mac/13.0`.
Rust: `https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html`, `https://doc.rust-lang.org/rustc/platform-support.html`, `https://static.rust-lang.org/dist/channel-rust-stable.toml`, `https://rust-lang.github.io/rustup/installation/other.html`, исходники `rust-lang/rust` тег `1.100.0` (`library/std/src/sys/fs/unix.rs`, `library/std/src/os/unix/net/ucred.rs`).
sing-box: `https://sing-box.sagernet.org/configuration/inbound/tun/`.
Крейты: `https://crates.io/api/v1/crates/<имя>`; исходники `https://static.crates.io/crates/<имя>/<имя>-<версия>.crate` (tokio, nix, rustix, system-configuration, io-kit-sys, interprocess, reqwest, hickory-resolver, rand, sha2, clap, thiserror, tempfile, uuid, ipnet, tracing-subscriber, time); `docs.rs` — `tokio/1.54.1` (`UnixStream::peer_cred`, `UCred`), `nix/0.31.3` (сборка 0.32.0 на docs.rs не удалась).
context7 в этой работе использовался только для поиска библиотек (resolve-library-id); содержательные сведения по API взяты из исходников крейтов.

---

## 11. Не подтверждено (и как проверить)

Системное, pf/launchd:

1. **Срабатывание правил в `com.apple/projectz`.** Вывод сделан из man (§1.2); без root `pfctl` не загрузить. Проверка на этапе 0: загрузить минимальный `block` и убедиться по счётчикам `pfctl -a com.apple/projectz -vsr`, что правило вычисляется и пакеты блокируются.
2. **Что `-a anchor -f` отвергает `set …`-опции** в файле anchor'а; в man опции описаны, а поведение в anchor'е прямо не названо.
3. **Что `-F all` с `-a com.apple/projectz` не затрагивает чужие anchor'ы**, и что `pfctl -F all` без `-a` сносит вложенные: man описывает `-a` как ограничитель, но явного абзаца нет.
4. **Формат вывода `pfctl -E`** (где печатается токен: stdout/stderr, строка `Token : N`). В `[M:pfctl]` формат не приведён. Использовать также `pfctl -s References`.
5. **`-T replace` с пустым списком** (допустим ли; иначе — `-T flush`).
6. **Сохраняются ли существующие состояния при перезагрузке/замене правил anchor'а** (и как лучше разрывать соединения: `-F states` затрагивает всю систему, `-k` — по хосту; фильтр по интерфейсу — не выяснено).
7. **DHCP со схемой `no state`** (ответ сервера сопоставляется правилом `pass in … port 67 → 68`) — проверить на реальной сети Wi-Fi и Ethernet.
8. **ARP/NDP и pf:** считается, что ARP не фильтруется; в man не сказано.
9. **AWDL/`awdl0`, `llw0`, `bridge0`:** трогать ли их правилами; включать ли `bridge0` (Thunderbolt Bridge) в `$phys`. По умолчанию — только аппаратные порты, через которые идёт маршрут по умолчанию.
10. **Права файла plist** (`root:wheel`, не доступен на запись остальным): общеизвестное требование launchd, в `launchd.plist(5)` отсутствует.
11. **Что при отсутствии `UserName` задание запускается от root:** man этого не говорит.
12. **`launch_activate_socket` из Rust:** наличие в `libc` не проверялось.
13. **Что переключатель в «Фоновых элементах» ставит именно `disallowed`/`disabled`** в `sfltool dumpbtm` и что `launchctl bootstrap` откажет для такого элемента — выведено из наблюдения за существующими записями; проверить на реальном `_projectz` helper'е, ручным выключением. Требуется ли root для `sfltool dumpbtm` в общем случае — на этой машине без sudo вывод был (792 строки), но в чужой конфигурации может быть иначе.
14. **Уведомление «Background Items Added»** при установке — на странице Apple не описано.
15. **`objc2-service-management` 0.3.2:** есть ли в нём `statusForLegacyPlist` — не смотрел.

Служебный пользователь:

16. **`IsHidden 1`** для системной учётки: существующая `_sntpd` его не имеет (`[L:]`); нужен ли он, чтобы пользователь не появился в окне входа, — не проверено.
17. **Свободный UID**: пересечение с будущими служебными пользователями Apple — рассуждение. Диапазон 292–394 свободен только на этой машине (13.7.8).
18. **Порядок setgroups/setgid/setuid** внутри std не документирован; проверять `ps`/`id` потомка.

Сеть/система:

19. **Переключение `Active` в `com.apple.timezone.auto.plist`**: достаточно ли `defaults write` от root, нужен ли перезапуск службы; поведение `systemsetup -settimezone` с IANA-именами и требование Full Disk Access для `systemsetup` в macOS 13 — не проверялись (`systemsetup` без root отказал).
20. **`killall -HUP mDNSResponder`** — man-страницы нет; взято из ТЗ.
21. **Определение Private Relay на Mac** (см. §6.2) — способа не найдено; поведение при VPN Apple на проверенных страницах не описала. Путь меню в System Settings — предположение.
22. **IOKit power notifications в демоне (LaunchDaemon без сессии пользователя):** API рассчитан на приложения; работоспособность в launchd-демоне не подтверждалась. Числовые значения `kIOMessage*` — по расчёту из макросов, сверить с `#define`.
23. **Привязка к интерфейсу:** что `reqwest::ClientBuilder::interface()` на macOS использует `IP_BOUND_IF`; что hickory-resolver умеет её (есть только `bind_addr`); достаточно ли `-ifscope`-маршрута. Нужен тест «с включённым TUN резолвер действительно уходит через en0».
24. **sha2 0.11 — формат результата** (`hex::encode`/`{:x}`): не проверялось компиляцией.
25. **`rust-toolchain.toml`, `MACOSX_DEPLOYMENT_TARGET`, `rustup target add`**: в документации страницы, прочитанные для справки, их не описывали; команды стандартные, но источника в этой справке нет.
26. **Edition 2024 «стабильна с 1.85»**: страница Edition Guide не отдала таблицу; вывод — по `edition = "2024"` и `rust-version = 1.85` у свежих крейтов.
27. **PID reuse / аудит-токен лучше PID** — общеизвестное соображение без источника в этой справке.
28. **Права и расположение сокета (`/var/run/projectz`)** — очищается ли `/var/run` при перезагрузке; готовое решение — создавать каталог при каждом старте.
29. **Код на Rust (§3.3, §4.2, §5.4, §8) не компилировался** — `rustc` отсутствует на машине; сигнатуры сверены с исходниками крейтов, но могут потребовать правок.
30. **Windows-крейт `windows` 0.62.2:** конкретные фичи и API WFP/служб не изучались (этап 2).
