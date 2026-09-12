```markdown
# Analysis Project — рефакторинг

## О проекте

Парсер логов биржевого приложения. Принимает поток байт (файл или `&[u8]`),
разбирает строки в типизированную модель (`LogLine`), умеет фильтровать по
режиму чтения и по `request_id`. Есть CLI-бинарь, который читает лог-файл
и печатает распарсенные строки.

## Что было сделано

Проект получен в виде прототипа с намеренно оставленными проблемами.
Проведён рефакторинг по 11 пунктам, перечисленным в задании. Логика и
поведение программы сохранены; тесты адаптированы под новые типы данных
(`NonZeroU32`, `Box<AuthData>`) и проходят.

### 1. Лишние вызовы `clone()` вместо ссылок

Сигнатура `Parser::parse` изменена с
`fn parse(&self, input: String) -> Result<(String, Dest), ()>` на
`fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Dest), ()>`.
Все комбинаторы (`Alt`, `Permutation`, `Delimited`, `Preceded`, `Map`,
`List`, `Take`, `KeyValue`, `StripWhitespace`, `Tag`, `QuotedTag`,
`Unquote`) переведены на работу с заимствованными срезами. Вызовы
`.clone()` и `.to_string()` внутри парсеров устранены.

### 2. `Rc<RefCell<T>>` там, где можно обойтись ссылками

`read_log` принимает `R: std::io::Read` по значению. `LogIterator`
параметризован `R: std::io::BufRead` и владеет `BufReader<R>`.
`Rc`, `RefCell`, `RefMutWrapper`, трейт `MyReader` удалены.

### 3. Циклы вместо итераторов

Фильтрация в `read_log` переписана на
`logs.filter(...).filter(...).collect()`. Вложенный цикл по `request_ids`
заменён на `request_ids.contains(...)`.

### 4. `unsafe` без необходимости

`unsafe { std::mem::transmute }` в `LogIterator::new` удалён вместе с
`RefCell` (см. п. 2). В проекте не осталось `unsafe`.

### 5. Singleton без необходимости

`LogLineParser` и `pub static LOG_LINE_PARSER` удалены. Введена свободная
функция `parse::parse_log_line(input: &str) -> Result<(&str, LogLine), ParseError>`.

### 6. Излишняя валидация вместо tight-типов

Проверки `if value == 0 { return Err(()) }` заменены на
`std::num::NonZeroU32` / `NonZeroI32`. `stdp::U32` теперь возвращает
`NonZeroU32`, `stdp::I32` — `NonZeroI32`. Поля `Backet::count`,
`UserCash::count`, `LogLine::request_id`, `authorized_capital`,
`liquidity` используют `NonZeroU32`.

### 7. Дублирование вместо дженериков

Шесть функций `just_parse_asset_dsc`, `just_parse_backet`,
`just_user_cash`, `just_user_backet`, `just_user_backets`,
`just_parse_anouncements` заменены одной дженерик-функцией
`pub fn just_parse<T: Parsable>(input: &str) -> Result<(&str, T), ParseError>`.

### 8. Трейт-объекты вместо дженериков

`Box<dyn MyReader>` в сигнатуре `read_log` и в `LogIterator` заменён на
дженерик-параметр `R: std::io::Read` / `R: std::io::BufRead`. Трейт
`MyReader` и `RefMutWrapper` удалены.

### 9. Последовательность `if` вместо `match`

Три `u8`-константы `READ_MODE_ALL` / `READ_MODE_ERRORS` /
`READ_MODE_EXCHANGES` заменены на `enum ReadMode { All, Errors, Exchanges }`
с методом `matches(&self, kind: &LogKind) -> bool`, реализованным через
`match`. В `Alt::parse` для 2/3/4/8 парсеров цепочки `if let Ok(...)`
заменены на `.or_else(...)`.

### 10. Enum с вариантом на несколько килобайт стека

`AppLogTraceKind::Connect(AuthData)` заменён на
`Connect(Box<AuthData>)`. Это уменьшает размер enum до размера
указателя, а не 1024 байт `AuthData`.

### 11. Паника вместо возврата ошибки

`read_log` возвращает `Result<Vec<LogLine>, ReadError>` вместо `Vec` с
`panic!` при неизвестном режиме. `main` возвращает
`Result<(), Box<dyn std::error::Error>>`; `unwrap()` и `panic!` заменены
на `?` и `ok_or`. Введён тип `parse::ParseError`, реализующий
`std::error::Error`, чтобы `?` работал в `main` без хаков вокруг `()`.

## Структура

- `src/parse.rs` — парсеры, комбинаторы, модель данных, `ParseError`.
- `src/lib.rs` — `read_log`, `LogIterator`, `ReadMode`, `ReadError`, тесты.
- `src/main.rs` — CLI-бинарь `cli`.
- `example.log` — пример лог-файла.

## Как запускать

```
cargo test -- --nocapture
cargo run example.log
```

`cargo test` — 18 тестов, все проходят.
`cargo run example.log` — печатает распарсенные строки, завершается с `Ok(())`.

## Примечания

- Тест-кейсы, работавшие с `u32`/`i32` и с `Connect(AuthData)`, поправлены
  на `NonZeroU32`/`NonZeroI32` и `Connect(Box<AuthData>)` — это разрешено
  заданием («тест-кейсы придётся поправить на другие типы данных»).
- Неиспользуемые, но оставленные для полноты API элементы (`AsIs`,
  `Either`, `Status`, `all3`, `all4`, `I32`, `quote`) помечены
  `#[allow(dead_code)]`, чтобы не засорять вывод сборки.
```