/// Трейт, чтобы **реализовывать** и **требовать** метод 'распарсь и покажи,
/// что распарсить осталось'
pub trait Parser {
    type Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()>;
}
/// Вспомогательный трейт, чтобы писать собственный десериализатор
pub trait Parsable: Sized {
    type Parser: Parser<Dest = Self>;
    fn parser() -> Self::Parser;
}

/// Ошибка парсинга.
/// Введена, чтобы `?` в `main` работал без хаков вокруг `()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError;

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error")
    }
}

impl std::error::Error for ParseError {}

pub mod stdp {
    use super::Parser;
    use std::num::{NonZeroI32, NonZeroU32};

    /// Беззнаковые числа
    #[derive(Debug)]
    pub struct U32;
    impl Parser for U32 {
        type Dest = NonZeroU32;
        fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
            let (remaining, is_hex) = input
                .strip_prefix("0x")
                .map_or((input, false), |remaining| (remaining, true));
            let end_idx = remaining
                .char_indices()
                .find_map(|(idx, c)| match (is_hex, c) {
                    (true, 'a'..='f' | '0'..='9' | 'A'..='F') => None,
                    (false, '0'..='9') => None,
                    _ => Some(idx),
                })
                .unwrap_or(remaining.len());
            let value = u32::from_str_radix(&remaining[..end_idx], if is_hex { 16 } else { 10 })
                .map_err(|_| ())?;
            let value = NonZeroU32::new(value).ok_or(())?;
            Ok((&remaining[end_idx..], value))
        }
    }
    /// Знаковые числа.
    /// Не используется в проде (только в тестах), но оставлен для полноты API.
    #[allow(dead_code)]
    #[derive(Debug)]
    pub struct I32;
    #[allow(dead_code)]
    impl Parser for I32 {
        type Dest = NonZeroI32;
        fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
            let end_idx = input
                .char_indices()
                .skip(1)
                .find_map(|(idx, c)| (!c.is_ascii_digit()).then_some(idx))
                .unwrap_or(input.len());
            let value: i32 = input[..end_idx].parse().map_err(|_| ())?;
            let value = NonZeroI32::new(value).ok_or(())?;
            Ok((&input[end_idx..], value))
        }
    }
    /// Шестнадцатеричные байты (пригодится при парсинге блобов)
    #[derive(Debug, Clone)]
    pub struct Byte;
    impl Parser for Byte {
        type Dest = u8;
        fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
            let (to_parse, remaining) = input.split_at_checked(2).ok_or(())?;
            if !to_parse.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(());
            }
            let value = u8::from_str_radix(to_parse, 16).map_err(|_| ())?;
            Ok((remaining, value))
        }
    }
}

/// Обернуть строку в кавычки, экранировав кавычки, которые в строке уже есть.
/// Используется только в тестах и потому не является частью публичного API.
#[allow(dead_code)]
fn quote(input: &str) -> String {
    let mut result = String::from("\"");
    result.extend(
        input
            .chars()
            .map(|c| match c {
                '\\' | '"' => ['\\', c].into_iter().take(2),
                _ => [c, ' '].into_iter().take(1),
            })
            .flatten(),
    );
    result.push('"');
    result
}
/// Распарсить строку, которую ранее [обернули в кавычки](quote)
/// Распарсить строку, которую ранее [обернули в кавычки](quote)
fn do_unquote(input: &str) -> Result<(&str, String), ()> {
    let mut result = String::new();
    let mut escaped_now = false;
    let mut iter = input.char_indices();
    // пропускаем ведущую кавычку
    match iter.next() {
        Some((_, '"')) => {}
        _ => return Err(()),
    }
    while let Some((idx, c)) = iter.next() {
        match (c, escaped_now) {
            ('"' | '\\', true) => {
                result.push(c);
                escaped_now = false;
            }
            ('\\', false) => escaped_now = true,
            ('"', false) => {
                let rest_start = idx + c.len_utf8();
                return Ok((&input[rest_start..], result));
            }
            (c, _) => {
                result.push(c);
                escaped_now = false;
            }
        }
    }
    Err(()) // строка кончилась, не закрыв кавычку
}
/// Распарсить строку, обёрную в кавычки (без вложенных кавычек)
fn do_unquote_non_escaped(input: &str) -> Result<(&str, String), ()> {
    let input = input.strip_prefix("\"").ok_or(())?;
    let quote_byteidx = input.find('"').ok_or(())?;
    if 0 == quote_byteidx || Some("\\") == input.get(quote_byteidx - 1..quote_byteidx) {
        return Err(());
    }
    Ok((
        &input[1 + quote_byteidx..],
        input[..quote_byteidx].to_string(),
    ))
}
/// Парсер кавычек
#[derive(Debug, Clone)]
pub struct Unquote;
impl Parser for Unquote {
    type Dest = String;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        do_unquote(input)
    }
}
/// Конструктор [Unquote]
pub fn unquote() -> Unquote {
    Unquote
}
/// Парсер, возвращающий результат как есть.
/// Не используется в проде, оставлен для полноты API.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AsIs;
#[allow(dead_code)]
impl Parser for AsIs {
    type Dest = String;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        Ok((&input[input.len()..], input.to_string()))
    }
}
/// Парсер константных строк
#[derive(Debug, Clone)]
pub struct Tag {
    pub tag: &'static str,
}
impl Parser for Tag {
    type Dest = ();
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        Ok((input.strip_prefix(self.tag).ok_or(())?, ()))
    }
}
/// Конструктор [Tag]
pub fn tag(tag: &'static str) -> Tag {
    Tag { tag }
}
/// Парсер [тэга](Tag), обёрнутого в кавычки
#[derive(Debug, Clone)]
pub struct QuotedTag(pub Tag);
impl Parser for QuotedTag {
    type Dest = ();
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let (remaining, candidate) = do_unquote_non_escaped(input)?;
        if !self.0.parse(&candidate)?.0.is_empty() {
            return Err(());
        }
        Ok((remaining, ()))
    }
}
/// Конструктор [QuotedTag]
pub fn quoted_tag(tag: &'static str) -> QuotedTag {
    QuotedTag(Tag { tag })
}
/// Комбинатор, пробрасывающий строку без лидирующих пробелов
#[derive(Debug, Clone)]
pub struct StripWhitespace<T> {
    pub parser: T,
}
impl<T: Parser> Parser for StripWhitespace<T> {
    type Dest = T::Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser
            .parse(input.trim_start())
            .map(|(remaining, parsed)| (remaining.trim_start(), parsed))
    }
}
/// Конструктор [StripWhitespace]
pub fn strip_whitespace<T: Parser>(parser: T) -> StripWhitespace<T> {
    StripWhitespace { parser }
}
/// Комбинатор `delimited`
#[derive(Debug, Clone)]
pub struct Delimited<Prefix, T, Suffix> {
    pub prefix_to_ignore: Prefix,
    pub dest_parser: T,
    pub suffix_to_ignore: Suffix,
}
impl<Prefix, T, Suffix> Parser for Delimited<Prefix, T, Suffix>
where
    Prefix: Parser,
    T: Parser,
    Suffix: Parser,
{
    type Dest = T::Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let (remaining, _) = self.prefix_to_ignore.parse(input)?;
        let (remaining, result) = self.dest_parser.parse(remaining)?;
        self.suffix_to_ignore
            .parse(remaining)
            .map(|(remaining, _)| (remaining, result))
    }
}
/// Конструктор [Delimited]
pub fn delimited<Prefix, T, Suffix>(
    prefix_to_ignore: Prefix,
    dest_parser: T,
    suffix_to_ignore: Suffix,
) -> Delimited<Prefix, T, Suffix>
where
    Prefix: Parser,
    T: Parser,
    Suffix: Parser,
{
    Delimited {
        prefix_to_ignore,
        dest_parser,
        suffix_to_ignore,
    }
}
/// Комбинатор-отображение
#[derive(Debug, Clone)]
pub struct Map<T, M> {
    pub parser: T,
    pub map: M,
}
impl<T: Parser, Dest: Sized, M: Fn(T::Dest) -> Dest> Parser for Map<T, M> {
    type Dest = Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser
            .parse(input)
            .map(|(remaining, pre_result)| (remaining, (self.map)(pre_result)))
    }
}
/// Конструктор [Map]
pub fn map<T: Parser, Dest: Sized, M: Fn(T::Dest) -> Dest>(parser: T, map: M) -> Map<T, M> {
    Map { parser, map }
}
/// Комбинатор с отбрасываемым префиксом
#[derive(Debug, Clone)]
pub struct Preceded<Prefix, T> {
    pub prefix_to_ignore: Prefix,
    pub dest_parser: T,
}
impl<Prefix, T> Parser for Preceded<Prefix, T>
where
    Prefix: Parser,
    T: Parser,
{
    type Dest = T::Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let (remaining, _) = self.prefix_to_ignore.parse(input)?;
        self.dest_parser.parse(remaining)
    }
}
/// Конструктор [Preceded]
pub fn preceded<Prefix, T>(prefix_to_ignore: Prefix, dest_parser: T) -> Preceded<Prefix, T>
where
    Prefix: Parser,
    T: Parser,
{
    Preceded {
        prefix_to_ignore,
        dest_parser,
    }
}
/// Комбинатор, который требует, чтобы все дочерние парсеры отработали
#[derive(Debug, Clone)]
pub struct All<T> {
    pub parser: T,
}
impl<A0, A1> Parser for All<(A0, A1)>
where
    A0: Parser,
    A1: Parser,
{
    type Dest = (A0::Dest, A1::Dest);
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let (remaining, a0) = self.parser.0.parse(input)?;
        self.parser
            .1
            .parse(remaining)
            .map(|(remaining, a1)| (remaining, (a0, a1)))
    }
}
pub fn all2<A0: Parser, A1: Parser>(a0: A0, a1: A1) -> All<(A0, A1)> {
    All { parser: (a0, a1) }
}
impl<A0, A1, A2> Parser for All<(A0, A1, A2)>
where
    A0: Parser,
    A1: Parser,
    A2: Parser,
{
    type Dest = (A0::Dest, A1::Dest, A2::Dest);
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let (remaining, a0) = self.parser.0.parse(input)?;
        let (remaining, a1) = self.parser.1.parse(remaining)?;
        self.parser
            .2
            .parse(remaining)
            .map(|(remaining, a2)| (remaining, (a0, a1, a2)))
    }
}
#[allow(dead_code)]
pub fn all3<A0: Parser, A1: Parser, A2: Parser>(a0: A0, a1: A1, a2: A2) -> All<(A0, A1, A2)> {
    All {
        parser: (a0, a1, a2),
    }
}
impl<A0, A1, A2, A3> Parser for All<(A0, A1, A2, A3)>
where
    A0: Parser,
    A1: Parser,
    A2: Parser,
    A3: Parser,
{
    type Dest = (A0::Dest, A1::Dest, A2::Dest, A3::Dest);
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let (remaining, a0) = self.parser.0.parse(input)?;
        let (remaining, a1) = self.parser.1.parse(remaining)?;
        let (remaining, a2) = self.parser.2.parse(remaining)?;
        self.parser
            .3
            .parse(remaining)
            .map(|(remaining, a3)| (remaining, (a0, a1, a2, a3)))
    }
}
#[allow(dead_code)]
pub fn all4<A0: Parser, A1: Parser, A2: Parser, A3: Parser>(
    a0: A0,
    a1: A1,
    a2: A2,
    a3: A3,
) -> All<(A0, A1, A2, A3)> {
    All {
        parser: (a0, a1, a2, a3),
    }
}
/// Комбинатор `"ключ":значение,`
#[derive(Debug, Clone)]
pub struct KeyValue<T> {
    pub parser: Delimited<
        All<(StripWhitespace<QuotedTag>, StripWhitespace<Tag>)>,
        StripWhitespace<T>,
        StripWhitespace<Tag>,
    >,
}
impl<T> Parser for KeyValue<T>
where
    T: Parser,
{
    type Dest = T::Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser.parse(input)
    }
}
/// Конструктор [KeyValue]
pub fn key_value<T: Parser>(key: &'static str, value_parser: T) -> KeyValue<T> {
    KeyValue {
        parser: delimited(
            all2(strip_whitespace(quoted_tag(key)), strip_whitespace(tag(":"))),
            strip_whitespace(value_parser),
            strip_whitespace(tag(",")),
        ),
    }
}
/// Комбинатор `permutation`
#[derive(Debug, Clone)]
pub struct Permutation<T> {
    pub parsers: T,
}
impl<A0, A1> Parser for Permutation<(A0, A1)>
where
    A0: Parser,
    A1: Parser,
{
    type Dest = (A0::Dest, A1::Dest);
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        match self.parsers.0.parse(input) {
            Ok((remaining, a0)) => self
                .parsers
                .1
                .parse(remaining)
                .map(|(remaining, a1)| (remaining, (a0, a1))),
            Err(()) => self.parsers.1.parse(input).and_then(|(remaining, a1)| {
                self.parsers
                    .0
                    .parse(remaining)
                    .map(|(remaining, a0)| (remaining, (a0, a1)))
            }),
        }
    }
}
pub fn permutation2<A0: Parser, A1: Parser>(a0: A0, a1: A1) -> Permutation<(A0, A1)> {
    Permutation { parsers: (a0, a1) }
}
impl<A0, A1, A2> Parser for Permutation<(A0, A1, A2)>
where
    A0: Parser,
    A1: Parser,
    A2: Parser,
{
    type Dest = (A0::Dest, A1::Dest, A2::Dest);
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        match self.parsers.0.parse(input) {
            Ok((remaining, a0)) => match self.parsers.1.parse(remaining) {
                Ok((remaining, a1)) => self
                    .parsers
                    .2
                    .parse(remaining)
                    .map(|(remaining, a2)| (remaining, (a0, a1, a2))),
                Err(()) => self.parsers.2.parse(remaining).and_then(|(remaining, a2)| {
                    self.parsers
                        .1
                        .parse(remaining)
                        .map(|(remaining, a1)| (remaining, (a0, a1, a2)))
                }),
            },
            Err(()) => match self.parsers.1.parse(input) {
                Ok((remaining, a1)) => match self.parsers.0.parse(remaining) {
                    Ok((remaining, a0)) => self
                        .parsers
                        .2
                        .parse(remaining)
                        .map(|(remaining, a2)| (remaining, (a0, a1, a2))),
                    Err(()) => self.parsers.2.parse(remaining).and_then(|(remaining, a2)| {
                        self.parsers
                            .0
                            .parse(remaining)
                            .map(|(remaining, a0)| (remaining, (a0, a1, a2)))
                    }),
                },
                Err(()) => self.parsers.2.parse(input).and_then(|(remaining, a2)| {
                    match self.parsers.0.parse(remaining) {
                        Ok((remaining, a0)) => self
                            .parsers
                            .1
                            .parse(remaining)
                            .map(|(remaining, a1)| (remaining, (a0, a1, a2))),
                        Err(()) => self.parsers.1.parse(remaining).and_then(|(remaining, a1)| {
                            self.parsers
                                .0
                                .parse(remaining)
                                .map(|(remaining, a0)| (remaining, (a0, a1, a2)))
                        }),
                    }
                }),
            },
        }
    }
}
pub fn permutation3<A0: Parser, A1: Parser, A2: Parser>(
    a0: A0,
    a1: A1,
    a2: A2,
) -> Permutation<(A0, A1, A2)> {
    Permutation {
        parsers: (a0, a1, a2),
    }
}
/// Комбинатор списка
#[derive(Debug, Clone)]
pub struct List<T> {
    pub parser: T,
}
impl<T: Parser> Parser for List<T> {
    type Dest = Vec<T::Dest>;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let mut remaining = input.trim_start().strip_prefix('[').ok_or(())?.trim_start();
        let mut result = Vec::new();
        loop {
            if let Some(rest) = remaining.strip_prefix(']') {
                return Ok((rest.trim_start(), result));
            }
            let (rest, item) = self.parser.parse(remaining)?;
            let rest = rest.trim_start().strip_prefix(',').ok_or(())?.trim_start();
            result.push(item);
            remaining = rest;
        }
    }
}
pub fn list<T: Parser>(parser: T) -> List<T> {
    List { parser }
}
/// Комбинатор `alt` через `or_else`
#[derive(Debug, Clone)]
pub struct Alt<T> {
    pub parser: T,
}
impl<A0, A1, Dest> Parser for Alt<(A0, A1)>
where
    A0: Parser<Dest = Dest>,
    A1: Parser<Dest = Dest>,
{
    type Dest = Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser
            .0
            .parse(input)
            .or_else(|_| self.parser.1.parse(input))
    }
}
pub fn alt2<Dest, A0: Parser<Dest = Dest>, A1: Parser<Dest = Dest>>(
    a0: A0,
    a1: A1,
) -> Alt<(A0, A1)> {
    Alt { parser: (a0, a1) }
}
impl<A0, A1, A2, Dest> Parser for Alt<(A0, A1, A2)>
where
    A0: Parser<Dest = Dest>,
    A1: Parser<Dest = Dest>,
    A2: Parser<Dest = Dest>,
{
    type Dest = Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser
            .0
            .parse(input)
            .or_else(|_| self.parser.1.parse(input))
            .or_else(|_| self.parser.2.parse(input))
    }
}
pub fn alt3<Dest, A0: Parser<Dest = Dest>, A1: Parser<Dest = Dest>, A2: Parser<Dest = Dest>>(
    a0: A0,
    a1: A1,
    a2: A2,
) -> Alt<(A0, A1, A2)> {
    Alt {
        parser: (a0, a1, a2),
    }
}
impl<A0, A1, A2, A3, Dest> Parser for Alt<(A0, A1, A2, A3)>
where
    A0: Parser<Dest = Dest>,
    A1: Parser<Dest = Dest>,
    A2: Parser<Dest = Dest>,
    A3: Parser<Dest = Dest>,
{
    type Dest = Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser
            .0
            .parse(input)
            .or_else(|_| self.parser.1.parse(input))
            .or_else(|_| self.parser.2.parse(input))
            .or_else(|_| self.parser.3.parse(input))
    }
}
pub fn alt4<
    Dest,
    A0: Parser<Dest = Dest>,
    A1: Parser<Dest = Dest>,
    A2: Parser<Dest = Dest>,
    A3: Parser<Dest = Dest>,
>(
    a0: A0,
    a1: A1,
    a2: A2,
    a3: A3,
) -> Alt<(A0, A1, A2, A3)> {
    Alt {
        parser: (a0, a1, a2, a3),
    }
}
impl<A0, A1, A2, A3, A4, A5, A6, A7, Dest> Parser for Alt<(A0, A1, A2, A3, A4, A5, A6, A7)>
where
    A0: Parser<Dest = Dest>,
    A1: Parser<Dest = Dest>,
    A2: Parser<Dest = Dest>,
    A3: Parser<Dest = Dest>,
    A4: Parser<Dest = Dest>,
    A5: Parser<Dest = Dest>,
    A6: Parser<Dest = Dest>,
    A7: Parser<Dest = Dest>,
{
    type Dest = Dest;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        self.parser
            .0
            .parse(input)
            .or_else(|_| self.parser.1.parse(input))
            .or_else(|_| self.parser.2.parse(input))
            .or_else(|_| self.parser.3.parse(input))
            .or_else(|_| self.parser.4.parse(input))
            .or_else(|_| self.parser.5.parse(input))
            .or_else(|_| self.parser.6.parse(input))
            .or_else(|_| self.parser.7.parse(input))
    }
}
pub fn alt8<
    Dest,
    A0: Parser<Dest = Dest>,
    A1: Parser<Dest = Dest>,
    A2: Parser<Dest = Dest>,
    A3: Parser<Dest = Dest>,
    A4: Parser<Dest = Dest>,
    A5: Parser<Dest = Dest>,
    A6: Parser<Dest = Dest>,
    A7: Parser<Dest = Dest>,
>(
    a0: A0,
    a1: A1,
    a2: A2,
    a3: A3,
    a4: A4,
    a5: A5,
    a6: A6,
    a7: A7,
) -> Alt<(A0, A1, A2, A3, A4, A5, A6, A7)> {
    Alt {
        parser: (a0, a1, a2, a3, a4, a5, a6, a7),
    }
}

/// Комбинатор для применения дочернего парсера N раз
#[derive(Debug, Clone)]
pub struct Take<T> {
    pub count: usize,
    pub parser: T,
}
impl<T: Parser> Parser for Take<T> {
    type Dest = Vec<T::Dest>;
    fn parse<'a>(&self, input: &'a str) -> Result<(&'a str, Self::Dest), ()> {
        let mut remaining = input;
        let mut result = Vec::new();
        for _ in 0..self.count {
            let (new_remaining, new_result) = self.parser.parse(remaining)?;
            result.push(new_result);
            remaining = new_remaining;
        }
        Ok((remaining, result))
    }
}
pub fn take<T: Parser>(count: usize, parser: T) -> Take<T> {
    Take { count, parser }
}

const AUTHDATA_SIZE: usize = 1024;

/// Данные для авторизации.
/// Хранятся в `Box`, чтобы не раздувать стек варианта enum, в котором лежат.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthData(pub [u8; AUTHDATA_SIZE]);
impl Parsable for AuthData {
    type Parser = Map<Take<stdp::Byte>, fn(Vec<u8>) -> Self>;
    fn parser() -> Self::Parser {
        map(take(AUTHDATA_SIZE, stdp::Byte), |authdata| {
            AuthData(authdata.try_into().unwrap_or([0; AUTHDATA_SIZE]))
        })
    }
}

/// Конструкция 'либо-либо'. Не используется, оставлена для полноты API.
#[allow(dead_code)]
pub enum Either<Left, Right> {
    Left(Left),
    Right(Right),
}

/// Статус, которые можно парсить. Не используется, оставлен для полноты API.
#[allow(dead_code)]
pub enum Status {
    Ok,
    Err(String),
}
#[allow(dead_code)]
impl Parsable for Status {
    type Parser = Alt<(
        Map<Tag, fn(()) -> Self>,
        Map<Delimited<Tag, Unquote, Tag>, fn(String) -> Self>,
    )>;
    fn parser() -> Self::Parser {
        fn to_ok(_: ()) -> Status {
            Status::Ok
        }
        fn to_err(error: String) -> Status {
            Status::Err(error)
        }
        alt2(
            map(tag("Ok"), to_ok),
            map(delimited(tag("Err("), unquote(), tag(")")), to_err),
        )
    }
}

/// Пара 'сокращённое название предмета' - 'его описание'
#[derive(Debug, Clone, PartialEq)]
pub struct AssetDsc {
    pub id: String,
    pub dsc: String,
}
impl Parsable for AssetDsc {
    type Parser = Map<
        Delimited<
            All<(StripWhitespace<Tag>, StripWhitespace<Tag>)>,
            Permutation<(KeyValue<Unquote>, KeyValue<Unquote>)>,
            StripWhitespace<Tag>,
        >,
        fn((String, String)) -> Self,
    >;
    fn parser() -> Self::Parser {
        map(
            delimited(
                all2(strip_whitespace(tag("AssetDsc")), strip_whitespace(tag("{"))),
                permutation2(key_value("id", unquote()), key_value("dsc", unquote())),
                strip_whitespace(tag("}")),
            ),
            |(id, dsc)| AssetDsc { id, dsc },
        )
    }
}
/// Сведение о предмете в некотором количестве
#[derive(Debug, Clone, PartialEq)]
pub struct Backet {
    pub asset_id: String,
    pub count: std::num::NonZeroU32,
}
impl Parsable for Backet {
    type Parser = Map<
        Delimited<
            All<(StripWhitespace<Tag>, StripWhitespace<Tag>)>,
            Permutation<(KeyValue<Unquote>, KeyValue<stdp::U32>)>,
            StripWhitespace<Tag>,
        >,
        fn((String, std::num::NonZeroU32)) -> Self,
    >;
    fn parser() -> Self::Parser {
        map(
            delimited(
                all2(strip_whitespace(tag("Backet")), strip_whitespace(tag("{"))),
                permutation2(key_value("asset_id", unquote()), key_value("count", stdp::U32)),
                strip_whitespace(tag("}")),
            ),
            |(asset_id, count)| Backet { asset_id, count },
        )
    }
}
/// Фиатные деньги конкретного пользователя
#[derive(Debug, Clone, PartialEq)]
pub struct UserCash {
    pub user_id: String,
    pub count: std::num::NonZeroU32,
}
impl Parsable for UserCash {
    type Parser = Map<
        Delimited<
            All<(StripWhitespace<Tag>, StripWhitespace<Tag>)>,
            Permutation<(KeyValue<Unquote>, KeyValue<stdp::U32>)>,
            StripWhitespace<Tag>,
        >,
        fn((String, std::num::NonZeroU32)) -> Self,
    >;
    fn parser() -> Self::Parser {
        map(
            delimited(
                all2(strip_whitespace(tag("UserCash")), strip_whitespace(tag("{"))),
                permutation2(key_value("user_id", unquote()), key_value("count", stdp::U32)),
                strip_whitespace(tag("}")),
            ),
            |(user_id, count)| UserCash { user_id, count },
        )
    }
}
/// [Backet] конкретного пользователя
#[derive(Debug, Clone, PartialEq)]
pub struct UserBacket {
    pub user_id: String,
    pub backet: Backet,
}
impl Parsable for UserBacket {
    type Parser = Map<
        Delimited<
            All<(StripWhitespace<Tag>, StripWhitespace<Tag>)>,
            Permutation<(KeyValue<Unquote>, KeyValue<<Backet as Parsable>::Parser>)>,
            StripWhitespace<Tag>,
        >,
        fn((String, Backet)) -> Self,
    >;
    fn parser() -> Self::Parser {
        map(
            delimited(
                all2(strip_whitespace(tag("UserBacket")), strip_whitespace(tag("{"))),
                permutation2(
                    key_value("user_id", unquote()),
                    key_value("backet", Backet::parser()),
                ),
                strip_whitespace(tag("}")),
            ),
            |(user_id, backet)| UserBacket { user_id, backet },
        )
    }
}
/// [Бакеты](Backet) конкретного пользователя
#[derive(Debug, Clone, PartialEq)]
pub struct UserBackets {
    pub user_id: String,
    pub backets: Vec<Backet>,
}
impl Parsable for UserBackets {
    type Parser = Map<
        Delimited<
            All<(StripWhitespace<Tag>, StripWhitespace<Tag>)>,
            Permutation<(KeyValue<Unquote>, KeyValue<List<<Backet as Parsable>::Parser>>)>,
            StripWhitespace<Tag>,
        >,
        fn((String, Vec<Backet>)) -> Self,
    >;
    fn parser() -> Self::Parser {
        map(
            delimited(
                all2(strip_whitespace(tag("UserBackets")), strip_whitespace(tag("{"))),
                permutation2(
                    key_value("user_id", unquote()),
                    key_value("backets", list(Backet::parser())),
                ),
                strip_whitespace(tag("}")),
            ),
            |(user_id, backets)| UserBackets { user_id, backets },
        )
    }
}
/// Список опубликованных бакетов
#[derive(Debug, Clone, PartialEq)]
pub struct Announcements(pub Vec<UserBackets>);
impl Parsable for Announcements {
    type Parser = Map<List<<UserBackets as Parsable>::Parser>, fn(Vec<UserBackets>) -> Self>;
    fn parser() -> Self::Parser {
        fn from_vec(vec: Vec<UserBackets>) -> Announcements {
            Announcements(vec)
        }
        map(list(UserBackets::parser()), from_vec)
    }
}

/// Одна дженерик-обёртка вместо шести функций `just_parse_*`
pub fn just_parse<T: Parsable>(input: &str) -> Result<(&str, T), ParseError> {
    T::parser().parse(input).map_err(|_| ParseError)
}

/// Все виды логов
#[derive(Debug, Clone, PartialEq)]
pub enum LogKind {
    System(SystemLogKind),
    App(AppLogKind),
}
/// Все виды [системных](LogKind) логов
#[derive(Debug, Clone, PartialEq)]
pub enum SystemLogKind {
    Error(SystemLogErrorKind),
    Trace(SystemLogTraceKind),
}
/// Trace [системы](SystemLogKind)
#[derive(Debug, Clone, PartialEq)]
pub enum SystemLogTraceKind {
    SendRequest(String),
    GetResponse(String),
}
/// Error [системы](SystemLogKind)
#[derive(Debug, Clone, PartialEq)]
pub enum SystemLogErrorKind {
    NetworkError(String),
    AccessDenied(String),
}
/// Все виды [логов приложения](LogKind) логов
#[derive(Debug, Clone, PartialEq)]
pub enum AppLogKind {
    Error(AppLogErrorKind),
    Trace(AppLogTraceKind),
    Journal(AppLogJournalKind),
}
/// Error [приложения](AppLogKind)
#[derive(Debug, Clone, PartialEq)]
pub enum AppLogErrorKind {
    LackOf(String),
    SystemError(String),
}
/// Trace [приложения](AppLogKind).
/// `AuthData` завёрнут в `Box`, чтобы вариант `Connect` не раздувал стек всего enum.
#[derive(Debug, Clone, PartialEq)]
pub enum AppLogTraceKind {
    Connect(Box<AuthData>),
    SendRequest(String),
    Check(Announcements),
    GetResponse(String),
}
/// Журнал [приложения](AppLogKind), самые высокоуровневые события
#[derive(Debug, Clone, PartialEq)]
pub enum AppLogJournalKind {
    CreateUser {
        user_id: String,
        authorized_capital: std::num::NonZeroU32,
    },
    DeleteUser {
        user_id: String,
    },
    RegisterAsset {
        asset_id: String,
        user_id: String,
        liquidity: std::num::NonZeroU32,
    },
    UnregisterAsset {
        asset_id: String,
        user_id: String,
    },
    DepositCash(UserCash),
    WithdrawCash(UserCash),
    BuyAsset(UserBacket),
    SellAsset(UserBacket),
}
impl Parsable for SystemLogErrorKind {
    type Parser = Preceded<
        Tag,
        Alt<(
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> SystemLogErrorKind>,
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> SystemLogErrorKind>,
        )>,
    >;
    fn parser() -> Self::Parser {
        preceded(
            tag("Error"),
            alt2(
                map(
                    preceded(strip_whitespace(tag("NetworkError")), strip_whitespace(unquote())),
                    |error| SystemLogErrorKind::NetworkError(error),
                ),
                map(
                    preceded(strip_whitespace(tag("AccessDenied")), strip_whitespace(unquote())),
                    |error| SystemLogErrorKind::AccessDenied(error),
                ),
            ),
        )
    }
}
impl Parsable for SystemLogTraceKind {
    type Parser = Preceded<
        Tag,
        Alt<(
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> SystemLogTraceKind>,
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> SystemLogTraceKind>,
        )>,
    >;
    fn parser() -> Self::Parser {
        preceded(
            tag("Trace"),
            alt2(
                map(
                    preceded(strip_whitespace(tag("SendRequest")), strip_whitespace(unquote())),
                    |request| SystemLogTraceKind::SendRequest(request),
                ),
                map(
                    preceded(strip_whitespace(tag("GetResponse")), strip_whitespace(unquote())),
                    |response| SystemLogTraceKind::GetResponse(response),
                ),
            ),
        )
    }
}
impl Parsable for SystemLogKind {
    type Parser = StripWhitespace<
        Preceded<
            Tag,
            Alt<(
                Map<<SystemLogTraceKind as Parsable>::Parser, fn(SystemLogTraceKind) -> SystemLogKind>,
                Map<<SystemLogErrorKind as Parsable>::Parser, fn(SystemLogErrorKind) -> SystemLogKind>,
            )>,
        >,
    >;
    fn parser() -> Self::Parser {
        strip_whitespace(preceded(
            tag("System::"),
            alt2(
                map(SystemLogTraceKind::parser(), |trace| SystemLogKind::Trace(trace)),
                map(SystemLogErrorKind::parser(), |error| SystemLogKind::Error(error)),
            ),
        ))
    }
}
impl Parsable for AppLogErrorKind {
    type Parser = Preceded<
        Tag,
        Alt<(
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> AppLogErrorKind>,
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> AppLogErrorKind>,
        )>,
    >;
    fn parser() -> Self::Parser {
        preceded(
            tag("Error"),
            alt2(
                map(
                    preceded(strip_whitespace(tag("LackOf")), strip_whitespace(unquote())),
                    |error| AppLogErrorKind::LackOf(error),
                ),
                map(
                    preceded(strip_whitespace(tag("SystemError")), strip_whitespace(unquote())),
                    |error| AppLogErrorKind::SystemError(error),
                ),
            ),
        )
    }
}
impl Parsable for AppLogTraceKind {
    type Parser = Preceded<
        Tag,
        Alt<(
            Map<
                Preceded<StripWhitespace<Tag>, StripWhitespace<<AuthData as Parsable>::Parser>>,
                fn(AuthData) -> AppLogTraceKind,
            >,
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> AppLogTraceKind>,
            Map<
                Preceded<StripWhitespace<Tag>, StripWhitespace<<Announcements as Parsable>::Parser>>,
                fn(Announcements) -> AppLogTraceKind,
            >,
            Map<Preceded<StripWhitespace<Tag>, StripWhitespace<Unquote>>, fn(String) -> AppLogTraceKind>,
        )>,
    >;
    fn parser() -> Self::Parser {
        preceded(
            tag("Trace"),
            alt4(
                map(
                    preceded(strip_whitespace(tag("Connect")), strip_whitespace(AuthData::parser())),
                    |authdata| AppLogTraceKind::Connect(Box::new(authdata)),
                ),
                map(
                    preceded(strip_whitespace(tag("SendRequest")), strip_whitespace(unquote())),
                    |trace| AppLogTraceKind::SendRequest(trace),
                ),
                map(
                    preceded(strip_whitespace(tag("Check")), strip_whitespace(Announcements::parser())),
                    |announcements| AppLogTraceKind::Check(announcements),
                ),
                map(
                    preceded(strip_whitespace(tag("GetResponse")), strip_whitespace(unquote())),
                    |trace| AppLogTraceKind::GetResponse(trace),
                ),
            ),
        )
    }
}
impl Parsable for AppLogJournalKind {
    type Parser = Preceded<
        Tag,
        Alt<(
            Map<
                Preceded<
                    StripWhitespace<Tag>,
                    Delimited<Tag, Permutation<(KeyValue<Unquote>, KeyValue<stdp::U32>)>, Tag>,
                >,
                fn((String, std::num::NonZeroU32)) -> AppLogJournalKind,
            >,
            Map<
                Preceded<StripWhitespace<Tag>, Delimited<Tag, KeyValue<Unquote>, Tag>>,
                fn(String) -> AppLogJournalKind,
            >,
            Map<
                Preceded<
                    StripWhitespace<Tag>,
                    Delimited<
                        Tag,
                        Permutation<(KeyValue<Unquote>, KeyValue<Unquote>, KeyValue<stdp::U32>)>,
                        Tag,
                    >,
                >,
                fn((String, String, std::num::NonZeroU32)) -> AppLogJournalKind,
            >,
            Map<
                Preceded<
                    StripWhitespace<Tag>,
                    Delimited<Tag, Permutation<(KeyValue<Unquote>, KeyValue<Unquote>)>, Tag>,
                >,
                fn((String, String)) -> AppLogJournalKind,
            >,
            Map<Preceded<StripWhitespace<Tag>, <UserCash as Parsable>::Parser>, fn(UserCash) -> AppLogJournalKind>,
            Map<Preceded<StripWhitespace<Tag>, <UserCash as Parsable>::Parser>, fn(UserCash) -> AppLogJournalKind>,
            Map<Preceded<StripWhitespace<Tag>, <UserBacket as Parsable>::Parser>, fn(UserBacket) -> AppLogJournalKind>,
            Map<Preceded<StripWhitespace<Tag>, <UserBacket as Parsable>::Parser>, fn(UserBacket) -> AppLogJournalKind>,
        )>,
    >;
    fn parser() -> Self::Parser {
        preceded(
            tag("Journal"),
            alt8(
                map(
                    preceded(
                        strip_whitespace(tag("CreateUser")),
                        delimited(
                            tag("{"),
                            permutation2(
                                key_value("user_id", unquote()),
                                key_value("authorized_capital", stdp::U32),
                            ),
                            tag("}"),
                        ),
                    ),
                    |(user_id, authorized_capital)| AppLogJournalKind::CreateUser {
                        user_id,
                        authorized_capital,
                    },
                ),
                map(
                    preceded(
                        strip_whitespace(tag("DeleteUser")),
                        delimited(tag("{"), key_value("user_id", unquote()), tag("}")),
                    ),
                    |user_id| AppLogJournalKind::DeleteUser { user_id },
                ),
                map(
                    preceded(
                        strip_whitespace(tag("RegisterAsset")),
                        delimited(
                            tag("{"),
                            permutation3(
                                key_value("asset_id", unquote()),
                                key_value("user_id", unquote()),
                                key_value("liquidity", stdp::U32),
                            ),
                            tag("}"),
                        ),
                    ),
                    |(asset_id, user_id, liquidity)| AppLogJournalKind::RegisterAsset {
                        asset_id,
                        user_id,
                        liquidity,
                    },
                ),
                map(
                    preceded(
                        strip_whitespace(tag("UnregisterAsset")),
                        delimited(
                            tag("{"),
                            permutation2(
                                key_value("asset_id", unquote()),
                                key_value("user_id", unquote()),
                            ),
                            tag("}"),
                        ),
                    ),
                    |(asset_id, user_id)| AppLogJournalKind::UnregisterAsset { asset_id, user_id },
                ),
                map(
                    preceded(strip_whitespace(tag("DepositCash")), UserCash::parser()),
                    |user_cash| AppLogJournalKind::DepositCash(user_cash),
                ),
                map(
                    preceded(strip_whitespace(tag("WithdrawCash")), UserCash::parser()),
                    |user_cash| AppLogJournalKind::WithdrawCash(user_cash),
                ),
                map(
                    preceded(strip_whitespace(tag("BuyAsset")), UserBacket::parser()),
                    |user_backet| AppLogJournalKind::BuyAsset(user_backet),
                ),
                map(
                    preceded(strip_whitespace(tag("SellAsset")), UserBacket::parser()),
                    |user_backet| AppLogJournalKind::SellAsset(user_backet),
                ),
            ),
        )
    }
}
impl Parsable for AppLogKind {
    type Parser = StripWhitespace<
        Preceded<
            Tag,
            Alt<(
                Map<<AppLogErrorKind as Parsable>::Parser, fn(AppLogErrorKind) -> AppLogKind>,
                Map<<AppLogTraceKind as Parsable>::Parser, fn(AppLogTraceKind) -> AppLogKind>,
                Map<<AppLogJournalKind as Parsable>::Parser, fn(AppLogJournalKind) -> AppLogKind>,
            )>,
        >,
    >;
    fn parser() -> Self::Parser {
        strip_whitespace(preceded(
            tag("App::"),
            alt3(
                map(AppLogErrorKind::parser(), |error| AppLogKind::Error(error)),
                map(AppLogTraceKind::parser(), |trace| AppLogKind::Trace(trace)),
                map(AppLogJournalKind::parser(), |journal| AppLogKind::Journal(journal)),
            ),
        ))
    }
}
impl Parsable for LogKind {
    type Parser = StripWhitespace<
        Alt<(
            Map<<SystemLogKind as Parsable>::Parser, fn(SystemLogKind) -> LogKind>,
            Map<<AppLogKind as Parsable>::Parser, fn(AppLogKind) -> LogKind>,
        )>,
    >;
    fn parser() -> Self::Parser {
        strip_whitespace(alt2(
            map(SystemLogKind::parser(), |system| LogKind::System(system)),
            map(AppLogKind::parser(), |app| LogKind::App(app)),
        ))
    }
}
/// Строка логов, [лог](AppLogKind) с `request_id`
#[derive(Debug, Clone, PartialEq)]
pub struct LogLine {
    pub kind: LogKind,
    pub request_id: std::num::NonZeroU32,
}
impl Parsable for LogLine {
    type Parser = Map<
        All<(
            <LogKind as Parsable>::Parser,
            StripWhitespace<Preceded<Tag, stdp::U32>>,
        )>,
        fn((LogKind, std::num::NonZeroU32)) -> Self,
    >;
    fn parser() -> Self::Parser {
        map(
            all2(
                LogKind::parser(),
                strip_whitespace(preceded(tag("requestid="), stdp::U32)),
            ),
            |(kind, request_id)| LogLine { kind, request_id },
        )
    }
}

/// Парсер одной строки лога.
/// Вместо singleton'а `LOG_LINE_PARSER` — просто функция: парсеры дёшевы в сборке.
pub fn parse_log_line(input: &str) -> Result<(&str, LogLine), ParseError> {
    <LogLine as Parsable>::parser()
        .parse(input)
        .map_err(|_| ParseError)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::num::{NonZeroI32, NonZeroU32};

    fn nz(v: u32) -> NonZeroU32 {
        NonZeroU32::new(v).unwrap()
    }

    #[test]
    fn test_u32() {
        assert_eq!(stdp::U32.parse("411").map(|(r, v)| (r, v.get())), Ok(("", 411)));
        assert_eq!(stdp::U32.parse("411ab").map(|(r, v)| (r, v.get())), Ok(("ab", 411)));
        assert_eq!(stdp::U32.parse(""), Err(()));
        assert_eq!(stdp::U32.parse("-3"), Err(()));
        assert_eq!(stdp::U32.parse("0x03").map(|(r, v)| (r, v.get())), Ok(("", 0x3)));
        assert_eq!(stdp::U32.parse("0x03abg").map(|(r, v)| (r, v.get())), Ok(("g", 0x3ab)));
        assert_eq!(stdp::U32.parse("0x"), Err(()));
        assert_eq!(stdp::U32.parse("0"), Err(())); // tight-тип отсекает ноль
    }

    #[test]
    fn test_i32() {
        assert_eq!(stdp::I32.parse("411").map(|(r, v)| (r, v.get())), Ok(("", 411)));
        assert_eq!(stdp::I32.parse("411ab").map(|(r, v)| (r, v.get())), Ok(("ab", 411)));
        assert_eq!(stdp::I32.parse(""), Err(()));
        assert_eq!(stdp::I32.parse("-3").map(|(r, v)| (r, v.get())), Ok(("", -3)));
        assert_eq!(stdp::I32.parse("0x03"), Err(()));
        assert_eq!(stdp::I32.parse("-"), Err(()));
        assert_eq!(stdp::I32.parse("0"), Err(()));
        let _ = NonZeroI32::new(1);
    }

    #[test]
    fn test_quote() {
        assert_eq!(quote(r#"411"#), r#""411""#.to_string());
        assert_eq!(quote(r#"4\11""#), r#""4\\11\"""#.to_string());
    }

    #[test]
    fn test_do_unquote_non_escaped() {
        assert_eq!(
            do_unquote_non_escaped(r#""411""#),
            Ok(("", "411".to_string()))
        );
        assert_eq!(do_unquote_non_escaped(r#" "411""#), Err(()));
        assert_eq!(do_unquote_non_escaped(r#"411"#), Err(()));
    }

    #[test]
    fn test_unquote() {
        assert_eq!(Unquote.parse(r#""411""#), Ok(("", "411".to_string())));
        assert_eq!(Unquote.parse(r#" "411""#), Err(()));
        assert_eq!(Unquote.parse(r#"411"#), Err(()));

        assert_eq!(
            Unquote.parse(r#""ni\\c\"e""#),
            Ok(("", r#"ni\c"e"#.to_string()))
        );
    }

    #[test]
    fn test_tag() {
        assert_eq!(tag("key=").parse("key=value"), Ok(("value", ())));
        assert_eq!(tag("key=").parse("key:value"), Err(()));
    }

    #[test]
    fn test_quoted_tag() {
        assert_eq!(quoted_tag("key").parse(r#""key"=value"#), Ok(("=value", ())));
        assert_eq!(quoted_tag("key").parse(r#""key:"value"#), Err(()));
        assert_eq!(quoted_tag("key").parse(r#"key=value"#), Err(()));
    }

    #[test]
    fn test_strip_whitespace() {
        assert_eq!(
            strip_whitespace(tag("hello")).parse(" hello world"),
            Ok(("world", ()))
        );
        assert_eq!(strip_whitespace(tag("hello")).parse("hello"), Ok(("", ())));
        assert_eq!(
            strip_whitespace(stdp::U32)
                .parse(" 42 answer")
                .map(|(r, v)| (r, v.get())),
            Ok(("answer", 42))
        );
    }

    #[test]
    fn test_delimited() {
        assert_eq!(
            delimited(tag("["), stdp::U32, tag("]"))
                .parse("[0x32]")
                .map(|(r, v)| (r, v.get())),
            Ok(("", 0x32))
        );
        assert_eq!(
            delimited(tag("["), stdp::U32, tag("]"))
                .parse("[0x32] nice")
                .map(|(r, v)| (r, v.get())),
            Ok((" nice", 0x32))
        );
        assert_eq!(delimited(tag("["), stdp::U32, tag("]")).parse("0x32]"), Err(()));
        assert_eq!(delimited(tag("["), stdp::U32, tag("]")).parse("[0x32"), Err(()));
    }

    #[test]
    fn test_key_value() {
        assert_eq!(
            key_value("key", stdp::U32)
                .parse(r#""key":32,"#)
                .map(|(r, v)| (r, v.get())),
            Ok(("", 32))
        );
        assert_eq!(key_value("key", stdp::U32).parse(r#"key:32,"#), Err(()));
        assert_eq!(key_value("key", stdp::U32).parse(r#""key":32"#), Err(()));
        assert_eq!(
            key_value("key", stdp::U32)
                .parse(r#" "key" : 32 , nice"#)
                .map(|(r, v)| (r, v.get())),
            Ok(("nice", 32))
        );
    }

    #[test]
    fn test_list() {
        assert_eq!(
            list(stdp::U32)
                .parse("[1,2,3,4,]")
                .map(|(r, v)| (r, v.into_iter().map(|x| x.get()).collect::<Vec<_>>())),
            Ok(("", vec![1, 2, 3, 4]))
        );
        assert_eq!(
            list(stdp::U32)
                .parse(" [ 1 , 2 , 3 , 4 , ] nice")
                .map(|(r, v)| (r, v.into_iter().map(|x| x.get()).collect::<Vec<_>>())),
            Ok(("nice", vec![1, 2, 3, 4]))
        );
        assert_eq!(list(stdp::U32).parse("1,2,3,4,"), Err(()));
        assert_eq!(
            list(stdp::U32)
                .parse("[]")
                .map(|(r, v)| (r, v.into_iter().map(|x| x.get()).collect::<Vec<_>>())),
            Ok(("", vec![]))
        );
    }

    #[test]
    fn test_authdata() {
        let s = "30c305825b900077ae7f8259c1c328aa3e124a07f3bfbbf216dfc6e308beea6e474b9a7ea6c24d003a6ae4fcf04a9e6ef7c7f17cdaa0296f66a88036badcf01f053da806fad356546349deceff24621b895440d05a715b221af8e9e068073d6dec04f148175717d3c2d1b6af84e2375718ab4a1eba7e037c1c1d43b4cf422d6f2aa9194266f0a7544eaeff8167f0e993d0ea6a8ddb98bfeb8805635d5ea9f6592fd5297e6f83b6834190f99449722cd0de87a4c122f08bbe836fd3092e5f0d37a3057e90f3dd41048da66cad3e8fd3ef72a9d86ecd9009c2db996af29dc62af5ef5eb04d0e16ce8fcecba92a4a9888f52d5d575e7dbc302ed97dbf69df15bb4f5c5601d38fbe3bd89d88768a6aed11ce2f95a6ad30bb72e787bfb734701cea1f38168be44ea19d3e98dd3c953fdb9951ac9c6e221bb0f980d8f0952ac8127da5bda7077dd25ffc8e1515c529f29516dacec6be9c084e6c91698267b2aed9038eca5ebafad479c5fb17652e25bb5b85586fae645bd7c3253d9916c0af65a20253412d5484ac15d288c6ca8823469090ded5ce0975dada63653797129f0e926af6247b457b067db683e37d848e0acf30e5602b78f1848e8da4b640ed08b75f3519a40ec96b2be964234beab37759504376c6e5ebfacdc57e4c7a22cf1e879d7bde29a2dca5fe20420215b59d102fd016606c533e8e36f7da114910664bade9b295d9043a01bc0dc4d8abbc16b1cec7789d89e699ad99dae597c7f10d6f047efc011d67444695cb8e6e8b3dba17ccc693729d01312d0f12a3fc76e12c2e4984af5cb3049b9d8a13124a1f770e96bae1fb153ba4c91bea4fae6f03010275d5a9b14012bdd678e037934dc6762005de54b32a7684e03060d5cc80378e9bef05b8f0692202944401bd06e4553e4490a0e57c5a72fc8abb1f714e22ea950fb2f1de284d6ff3da435954de355c677f60db4252a510919cbe7dadfed0441cf125fd8894753af8114f2ddacb75c3daa460920fc47d285e59fe9110e4151fcef03fa246cd2dd9a4d573e1dbbda1c6968cf4f546289b95ce1bf0a55eea6531382826d4002bc46bf441ce16056d42b5a2079e299e3191c23a7604cde03de6081e06f93cfe632c9a6088cd328662d47a4954934832df5b5f3765dbe136114c73c55cb7ce639e5d40d1d1d8f540d3c8e1bc7423f032c0da5264353468f009c973eec0448e41f9289e8d9dadc68da77d3c3ab3a6477d44024f21fba0bd4477d81c6027657527aa0413b45f417cb7b3beea835a1d5d795414d38156324cb5c1303e9924dbe40cd497c4c23c221cb912058c939bea8b79b3fea360fecaa83375a9a84e338d9e863e8021ad2df4430b8dea0c1714e1bdc478f559705549ad738453ab65c0ffcc8cf0e3bafaf4afad75ecc4dfad0de0cfe27d50d656456ea6c361b76508357714079424";
        let res = AuthData::parser().parse(s);
        assert!(res.is_ok());
        assert_eq!(res.as_ref().unwrap().0.len(), 0);
    }

    #[test]
    fn test_asset_dsc() {
        assert_eq!(
            all2(strip_whitespace(tag("AssetDsc")), strip_whitespace(tag("{")))
                .parse(" AssetDsc { "),
            Ok(("", ((), ())))
        );

        assert_eq!(
            AssetDsc::parser().parse(r#"AssetDsc{"id":"usd","dsc":"USA dollar",}"#),
            Ok((
                "",
                AssetDsc {
                    id: "usd".to_string(),
                    dsc: "USA dollar".to_string()
                }
            ))
        );
        assert_eq!(
            AssetDsc::parser().parse(r#" AssetDsc { "id" : "usd" , "dsc" : "USA dollar" , } "#),
            Ok((
                "",
                AssetDsc {
                    id: "usd".to_string(),
                    dsc: "USA dollar".to_string()
                }
            ))
        );
        assert_eq!(
            AssetDsc::parser()
                .parse(r#" AssetDsc { "id" : "usd" , "dsc" : "USA dollar" , } nice "#),
            Ok((
                "nice ",
                AssetDsc {
                    id: "usd".to_string(),
                    dsc: "USA dollar".to_string()
                }
            ))
        );

        assert_eq!(
            AssetDsc::parser().parse(r#"AssetDsc{"dsc":"USA dollar","id":"usd",}"#),
            Ok((
                "",
                AssetDsc {
                    id: "usd".to_string(),
                    dsc: "USA dollar".to_string()
                }
            ))
        );
    }

    #[test]
    fn test_backet() {
        assert_eq!(
            Backet::parser().parse(r#"Backet{"asset_id":"usd","count":42,}"#),
            Ok((
                "",
                Backet {
                    asset_id: "usd".to_string(),
                    count: nz(42)
                }
            ))
        );
        assert_eq!(
            Backet::parser().parse(r#"Backet{"count":42,"asset_id":"usd",}"#),
            Ok((
                "",
                Backet {
                    asset_id: "usd".to_string(),
                    count: nz(42)
                }
            ))
        );
    }

    #[test]
    fn test_log_kind() {
        assert_eq!(
            preceded(strip_whitespace(tag("NetworkError")), strip_whitespace(unquote()))
                .parse(r#"NetworkError "url unknown""#),
            Ok(("", "url unknown".to_string()))
        );

        assert_eq!(
            LogKind::parser().parse(r#"System::Error NetworkError "url unknown""#),
            Ok((
                "",
                LogKind::System(SystemLogKind::Error(SystemLogErrorKind::NetworkError(
                    "url unknown".to_string()
                )))
            ))
        );
        assert_eq!(
            LogKind::parser().parse(
                r#"App::Journal CreateUser {"user_id": "Steeve", "authorized_capital": 10000,}"#
            ),
            Ok((
                "",
                LogKind::App(AppLogKind::Journal(AppLogJournalKind::CreateUser {
                    user_id: "Steeve".to_string(),
                    authorized_capital: nz(10_000)
                }))
            ))
        );
        assert_eq!(
            LogKind::parser().parse(r#"App::Journal DeleteUser {"user_id": "Steeve",}"#),
            Ok((
                "",
                LogKind::App(AppLogKind::Journal(AppLogJournalKind::DeleteUser {
                    user_id: "Steeve".to_string()
                }))
            ))
        );
        assert_eq!(
            LogKind::parser().parse(
                r#"App::Journal RegisterAsset {"asset_id": "bayc", "liquidity": 100000000, "user_id": "Steeve",}"#
            ),
            Ok((
                "",
                LogKind::App(AppLogKind::Journal(AppLogJournalKind::RegisterAsset {
                    asset_id: "bayc".to_string(),
                    user_id: "Steeve".to_string(),
                    liquidity: nz(100_000_000)
                }))
            ))
        );
        assert_eq!(
            LogKind::parser()
                .parse(r#"App::Journal DepositCash UserCash{"user_id": "Steeve", "count": 10,}"#),
            Ok((
                "",
                LogKind::App(AppLogKind::Journal(AppLogJournalKind::DepositCash(
                    UserCash {
                        user_id: "Steeve".to_string(),
                        count: nz(10)
                    }
                )))
            ))
        );
        assert_eq!(
            LogKind::parser().parse(
                r#"App::Journal BuyAsset UserBacket{"user_id": "Steeve", "backet": Backet{"asset_id":"bayc","count":1,},}"#
            ),
            Ok((
                "",
                LogKind::App(AppLogKind::Journal(AppLogJournalKind::BuyAsset(
                    UserBacket {
                        user_id: "Steeve".to_string(),
                        backet: Backet {
                            asset_id: "bayc".to_string(),
                            count: nz(1)
                        }
                    }
                )))
            ))
        );
    }
}