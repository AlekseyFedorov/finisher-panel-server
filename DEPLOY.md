# Развёртывание

Схема: бинарник под systemd слушает локальный порт, nginx проксирует на него подпуть сайта.
Фронтенд обращается к `api/state` относительным путём, поэтому за прокси на подпути работает
без правок.

Ниже `<user>` — системный пользователь службы, `<домен>` — сайт, в который встраивается панель.

## Где что лежит

| | Путь |
|---|---|
| Каталог сборки | `~/finisher-panel-server` (клон репозитория) |
| Рабочий каталог | `/opt/finisher-panel` |
| Бинарник | `/opt/finisher-panel/finisher-panel-server` |
| Состояние | `/opt/finisher-panel/finisher-data.yaml` |
| Юнит | `/etc/systemd/system/finisher-panel.service` |

Состояние намеренно лежит вне каталога сборки: путь по умолчанию подставляется на этапе
компиляции (`CARGO_MANIFEST_DIR`), поэтому без `FINISHER_DATA` бинарник пишет туда, где шёл
`cargo build`, и данные теряются при пересборке в другом месте.

## Обновление

```bash
cd ~/finisher-panel-server
git pull
cargo build --release

D=$(date +%Y%m%d-%H%M%S)
cp -p /opt/finisher-panel/finisher-panel-server /opt/finisher-panel/finisher-panel-server.bak-$D
cp -p /opt/finisher-panel/finisher-data.yaml    /opt/finisher-panel/finisher-data.yaml.bak-$D

sudo systemctl stop finisher-panel
cp target/release/finisher-panel-server /opt/finisher-panel/
sudo systemctl start finisher-panel

systemctl is-active finisher-panel
curl -s -o /dev/null -w '%{http_code} %{size_download}\n' http://127.0.0.1:8081/api/state
```

Последняя строка должна вернуть `200` и ненулевой размер — это подтверждает, что новый бинарник
нашёл существующий файл состояния, а не начал с пустого.

Если SSH рвёт длинную сборку, запустить её отдельно от сессии:

```bash
setsid nohup bash -c 'cd ~/finisher-panel-server && git pull && cargo build --release && echo DONE' \
  > /tmp/fp-build.log 2>&1 < /dev/null &
tail -3 /tmp/fp-build.log   # позже
```

## Откат

```bash
sudo systemctl stop finisher-panel
cp /opt/finisher-panel/finisher-panel-server.bak-<метка> /opt/finisher-panel/finisher-panel-server
sudo systemctl start finisher-panel
```

Данные откатывать отдельно и только при необходимости — копировать соответствующий
`finisher-data.yaml.bak-<метка>` поверх рабочего файла при остановленной службе.

## Установка с нуля

```bash
# toolchain — в домашний каталог, без sudo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
sudo apt install -y build-essential git

git clone https://github.com/AlekseyFedorov/finisher-panel-server.git ~/finisher-panel-server
cd ~/finisher-panel-server && cargo build --release

sudo mkdir -p /opt/finisher-panel && sudo chown $USER:$USER /opt/finisher-panel
cp target/release/finisher-panel-server /opt/finisher-panel/
```

`/etc/systemd/system/finisher-panel.service`:

```ini
[Unit]
Description=Finisher panel server
After=network.target

[Service]
User=<user>
Group=<user>
WorkingDirectory=/opt/finisher-panel
Environment=FINISHER_ADDR=127.0.0.1:8081
Environment=FINISHER_DATA=/opt/finisher-panel/finisher-data.yaml
ExecStart=/opt/finisher-panel/finisher-panel-server
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Блок в `server`-секции конфига сайта `<домен>`:

```nginx
    location /finisher {
        return 301 /finisher/;
    }
    location /finisher/ {
        proxy_pass http://127.0.0.1:8081/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
```

Запуск:

```bash
sudo systemctl daemon-reload && sudo systemctl enable --now finisher-panel
sudo nginx -t && sudo systemctl reload nginx
```

## Две вещи, которые ломают развёртывание молча

**Завершающий слэш в `proxy_pass http://127.0.0.1:8081/;`.** Без него nginx передаёт префикс
`/finisher/` дальше, и все запросы упираются в 404.

**`Environment=FINISHER_DATA`.** Без него бинарник пишет в каталог сборки. Панель при этом
открывается и выглядит рабочей — просто пустой, а прежний журнал остаётся лежать нетронутым
в другом месте. `WorkingDirectory=` на выбор пути не влияет.
