# yatty M0: спайк рендера

> Для исполнения: выполнять задачи по порядку, отмечая чекбоксы. Спецификация: `docs/superpowers/specs/2026-11-08-myterm-design.md`.

**Цель M0:** одно окно на macOS, wgpu-рендер, один шелл с вводом и выводом, моноширинный шрифт с ligatures. Результат: решение «идём дальше / меняем стек» и замеры производительности.

**Вне M0:** табы, сплиты, профили, TOML-конфиг (кроме захардкоженных значений), поиск, мышь, подпись.

**Критерии готовности M0:**
- `cargo run -p app` открывает окно, в нём работает шелл (`zsh -l`), ввод и вывод корректны.
- Вывод `ls --color`, `vim` и `htop` отображается без артефактов в базовом виде.
- Строки `->`, `=>`, `!=` с ligature-шрифтом рисуются как лигатуры.
- Замерены: холодный старт до первого кадра, время кадра при выводе большого объёма, RSS в простое.
- Результаты записаны в `docs/superpowers/spikes/m0-results.md` с вердиктом.

---

## Структура

```
Cargo.toml                 # workspace
crates/
  term/src/lib.rs          # PTY + alacritty_terminal, Session
  render/src/lib.rs        # wgpu, атлас, шейпинг
  app/src/main.rs          # winit, цикл событий
  ui/src/lib.rs            # заглушка (пустой крейт)
  config/src/lib.rs        # заглушка (пустой крейт)
docs/superpowers/spikes/m0-results.md
```

`ui` и `config` в M0 создаются пустыми, чтобы зафиксировать границы из спецификации.

---

## Задача 1. Workspace и каркас крейтов

**Файлы:** `Cargo.toml`, `crates/*/Cargo.toml`, `crates/*/src/lib.rs`, `crates/app/src/main.rs`

- [ ] Создать корневой `Cargo.toml` с `[workspace]`, `members = ["crates/*"]`, `resolver = "2"`.
- [ ] Создать крейты `term`, `render`, `ui`, `config` (библиотеки) и `app` (бинарник).
- [ ] В `[workspace.dependencies]` зафиксировать версии: `alacritty_terminal`, `wgpu`, `winit`, `cosmic-text` (или `rustybuzz` + `fontdue`, выбор в задаче 4), `pollster`, `anyhow`, `tracing`, `tracing-subscriber`.
- [ ] Добавить `.gitignore` с `target/`.
- [ ] Проверка: `cargo build --workspace` проходит.
- [ ] Коммит: `chore: scaffold yatty workspace`.

## Задача 2. Терминальный бэкенд (`term`)

**Файлы:** `crates/term/src/lib.rs`, `crates/term/tests/pty.rs`

- [ ] Тип `Session`: создаёт PTY, запускает шелл из аргумента (по умолчанию `$SHELL -l`), поднимает `alacritty_terminal::Term` и `EventLoop`.
- [ ] Метод `write(&[u8])` для ввода, метод `snapshot()` возвращает сетку символов текущего экрана (символ, цвета, атрибуты, курсор).
- [ ] Уведомление о новом выводе через канал или callback для перерисовки.
- [ ] Тест: сессия запускает `/bin/echo hello`, в снимке сетки есть строка `hello`.
- [ ] Тест: после `exit` сессия сообщает код выхода.
- [ ] Проверка: `cargo test -p term`.
- [ ] Коммит: `feat(term): PTY session backed by alacritty_terminal`.

## Задача 3. Окно и цикл событий (`app`)

**Файлы:** `crates/app/src/main.rs`

- [ ] Открыть окно `winit` с заголовком `yatty` и размером 1100×700.
- [ ] Перенаправлять клавиатурный ввод в `Session::write`, включая Enter, Backspace, стрелки и Ctrl-комбинации.
- [ ] При изменении размера окна пересчитывать число колонок и строк и сообщать `Session` новый размер (PTY resize).
- [ ] Логировать через `tracing`.
- [ ] Проверка: окно открывается, ввод `ls` даёт вывод.
- [ ] Коммит: `feat(app): window and keyboard input`.

## Задача 4. Рендер текста (`render`)

**Файлы:** `crates/render/src/lib.rs`, `crates/render/src/atlas.rs`, `crates/render/src/shader.wgsl`

- [ ] Инициализация wgpu на Metal: instance, surface, adapter, device, swapchain.
- [ ] Загрузка моноширинного шрифта с Nerd Font (например, JetBrains Mono Nerd Font) через `cosmic-text`.
- [ ] Атлас глифов: растеризация по запросу, кэш по ключу (глиф, размер, стиль).
- [ ] Отрисовка сетки: фон ячеек, глифы, курсор (блок). Один draw call на кадр, если возможно.
- [ ] Цвета: 16 базовых, 256 индексированных, true color (RGB).
- [ ] Шейпинг: строки шейпятся целиком через `cosmic-text`, чтобы лигатуры работали. Если это ломает позиционирование ячеек, зафиксировать в результатах.
- [ ] Тест: `render` строит список глифов по эталонной сетке символов, снимок сохраняется в `crates/render/tests/snapshots/`.
- [ ] Проверка: `cargo test -p render`, визуально в окне `ls --color` и `vim`.
- [ ] Коммит: `feat(render): wgpu text grid with glyph atlas`.

## Задача 5. Интеграция и проверка ligatures

- [ ] Соединить `app` → `term::Session` → `render`: вывод шелла появляется в окне, курсор на месте.
- [ ] Вручную проверить: `echo '-> => != ==='`, `vim` (открыть и выйти), `htop`, `ls --color`, Nerd Font иконки (например, `echo ` ` в шрифте с Nerd Font).
- [ ] Зафиксировать результаты в заметке (см. задачу 6).
- [ ] Коммит: `feat: wire session to renderer`.

## Задача 6. Замеры и вердикт

**Файл:** `docs/superpowers/spikes/m0-results.md`

- [ ] Холодный старт до первого кадра: замерить 5 запусков, записать медиану. Цель: менее 200 мс.
- [ ] Время кадра при выводе большого объёма: `yes | head -n 200000` или `cat` большого файла. Записать p50 и p95. Цель: менее 16 мс на кадр без подвисаний.
- [ ] Память в простое (RSS) через `ps -o rss`. Цель ориентировочно: менее 100 МБ на вкладку.
- [ ] Таблица проверок: ligatures, Nerd Fonts, vim, htop, true color, resize.
- [ ] Вердикт: «идём дальше» или «меняем стек», с причиной. Если меняем, указать, что именно.
- [ ] Коммит: `docs: M0 spike results`.

---

## Риски, которые проверяет M0

- Ligatures и шейпинг в wgpu: если `cosmic-text` не даёт корректных позиций ячеек, нужен другой путь.
- Производительность атласа при большом выводе.
- Совместимость `alacritty_terminal` с выбранной версией winit и wgpu.

## После M0

Если вердикт положительный, пишем план M1 (true color, мышь, полный конфиг, ligatures в продакшн-виде). Если отрицательный, в спецификацию вносится правка по стеку до начала M1.
