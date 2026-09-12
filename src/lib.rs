pub mod parse;
use parse::*;

/// Режим чтения из логов.
/// Вместо трёх `u8`-констант — enum, чтобы компилятор проверял варианты.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    /// Читать всё подряд
    All,
    /// Читать только ошибки
    Errors,
    /// Читать только операции, касающиеся обменов
    Exchanges,
}

impl ReadMode {
    fn matches(&self, kind: &LogKind) -> bool {
        match self {
            ReadMode::All => true,
            ReadMode::Errors => matches!(
                kind,
                LogKind::System(SystemLogKind::Error(_)) | LogKind::App(AppLogKind::Error(_))
            ),
            ReadMode::Exchanges => matches!(
                kind,
                LogKind::App(AppLogKind::Journal(
                    AppLogJournalKind::BuyAsset(_)
                        | AppLogJournalKind::SellAsset(_)
                        | AppLogJournalKind::CreateUser { .. }
                        | AppLogJournalKind::RegisterAsset { .. }
                        | AppLogJournalKind::DepositCash(_)
                        | AppLogJournalKind::WithdrawCash(_)
                ))
            ),
        }
    }
}

/// Ошибка чтения логов
#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(e) => write!(f, "io error: {}", e),
        }
    }
}

impl std::error::Error for ReadError {}

impl From<std::io::Error> for ReadError {
    fn from(e: std::io::Error) -> Self {
        ReadError::Io(e)
    }
}

/// Итератор, на выходе которого — строки распарсенной структуры данных.
/// Дженерик вместо `Box<dyn MyReader>` + `Rc<RefCell<...>>`.
struct LogIterator<R: std::io::BufRead> {
    lines: std::io::Lines<R>,
}

impl<R: std::io::BufRead> LogIterator<R> {
    fn new(reader: R) -> Self {
        Self {
            lines: reader.lines(),
        }
    }
}

impl<R: std::io::BufRead> Iterator for LogIterator<R> {
    type Item = parse::LogLine;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = self.lines.next()?.ok()?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (remaining, result) = parse::parse_log_line(trimmed).ok()?;
            if remaining.trim().is_empty() {
                return Some(result);
            }
            // если строка распарсилась не полностью — пропускаем её
        }
    }
}

/// Принимает поток байт, отдаёт отфильтрованные и распарсенные логи.
/// Без `Rc`, `RefCell`, `unsafe`, singleton'а и паник.
pub fn read_log<R: std::io::Read>(
    input: R,
    mode: ReadMode,
    request_ids: &[u32],
) -> Result<Vec<LogLine>, ReadError> {
    let reader = std::io::BufReader::with_capacity(4096, input);
    let logs = LogIterator::new(reader);

    let collected: Vec<LogLine> = logs
        .filter(|log| request_ids.is_empty() || request_ids.contains(&log.request_id.get()))
        .filter(|log| mode.matches(&log.kind))
        .collect();

    Ok(collected)
}

#[cfg(test)]
mod test {
    use super::*;

    const SOURCE1: &str = r#"System::Error NetworkError "url unknown" requestid=1"#;

    const SOURCE: &str = r#"
System::Error NetworkError "network interface is down" requestid=1
App::Error SystemError "network" requestid=1
System::Trace SendRequest "CreateUser{\"user_id\": 10, \"authrized_capital\": 1000,}" requestid=2
System::Trace GetResponse "HTTP 401" requestid=2
System::Error AccessDenied "not authrized" requestid=2
App::Error SystemError "authorization" requestid=2

System::Trace SendRequest "login me" requestid=3
System::Trace SendRequest "login me" requestid=4
System::Trace GetResponse "HTTP 200" requestid=3
System::Trace GetResponse "HTTP 200" requestid=4
System::Trace SendRequest "Jupiter->CreateUser{\"user_id\": \"Bob\", \"authrized_capital\": 1000,}" requestid=4
System::Trace SendRequest "Jupiter->CreateUser{\"user_id\": \"Alice\", \"authrized_capital\": 5000,}" requestid=3
App::Trace SendRequest "CreateUser{\"user_id\": \"Bob\", \"authrized_capital\": 1000,}" requestid=4
App::Trace SendRequest "CreateUser{\"user_id\": \"Alice\", \"authrized_capital\": 5000,}" requestid=3
System::Trace GetResponse "HTTP 200" requestid=4
System::Trace GetResponse "HTTP 200" requestid=3
App::Trace GetResponse "Ok" requestid=4
App::Trace GetResponse "Ok" requestid=3
App::Journal CreateUser {"user_id": "Alice", "authorized_capital": 5000,} requestid=3
App::Journal CreateUser {"user_id": "Bob", "authorized_capital": 1000,} requestid=4

System::Trace SendRequest "Jupiter->RegisterAsset{\"asset_id\": \"milk\", \"user_id\": \"Bob\", \"liquidity\":10000,}" requestid=5
App::Trace SendRequest "RegisterAsset{\"asset_id\": \"milk\", \"user_id\": \"Bob\", \"liquidity\":10000,}" requestid=5
System::Trace GetResponse "HTTP 200" requestid=5
App::Trace GetResponse "Ok" requestid=5
App::Journal RegisterAsset {"asset_id": "milk", "user_id": "Bob", "liquidity": 10000,} requestid=5

System::Trace SendRequest "Jupiter->RegisterAsset{\"asset_id\": \"milk\", \"user_id\": \"Alice\", \"liquidity\":5000,}" requestid=6
App::Trace SendRequest "RegisterAsset{\"asset_id\": \"milk\", \"user_id\": \"Bob\", \"liquidity\":5000,}" requestid=6
System::Trace GetResponse "HTTP 200" requestid=6
App::Trace GetResponse "Ok" requestid=6
App::Journal RegisterAsset {"asset_id": "butter", "user_id": "Alice", "liquidity": 5000,} requestid=6

System::Error NetworkError "NS_BINDING_ABORTED" requestid=7
App::Error SystemError "network" requestid=7

System::Trace SendRequest "Jupiter->GetAnnouncements" requestid=8
App::Trace SendRequest "GetAnnouncements" requestid=8
System::Trace GetResponse "HTTP 200 []" requestid=8
App::Trace GetResponse "[]" requestid=8
App::Trace Check [] requestid=8
App::Error LackOf "can't buy milk, no sellers" requestid=8

System::Trace SendRequest "Jupiter->SellAsset" requestid=9
App::Trace SendRequest "SellAsset UserBacket{\"user_id\":\"Bob\",\"backet\":Backet{\"asset_id\":\"milk\",\"count\":3,},}" requestid=9
System::Trace GetResponse "HTTP 200" requestid=9
App::Journal SellAsset UserBacket{"user_id":"Bob","backet":Backet{"asset_id":"milk","count":3,},} requestid=9

System::Trace SendRequest "Jupiter->GetAnnouncements" requestid=10
App::Trace SendRequest "GetAnnouncements" requestid=10
System::Trace GetResponse "HTTP 200 [UserBackets{\"user_id\":\"Bob\",\"backets\":[Backet{\"asset_id\":\"milk\",\"count\":3,},],},]" requestid=10
App::Trace GetResponse "Ok" requestid=10
App::Trace Check [UserBackets{"user_id":"Bob","backets":[Backet{"asset_id":"milk","count":3,},],},] requestid=10
System::Trace SendRequest "Jupiter->buyasset{\"user_id\":\"alice\",\"backet\":backet{\"asset_id\":\"milk\",\"count\":5,},}" requestid=10
App::Trace SendRequest "{\"user_id\":\"Alice\",\"backet\":Backet{\"asset_id\":\"milk\",\"count\":5,},}" requestid=10
System::Trace GetResponse "HTTP 200" requestid=10
App::Trace GetResponse "Ok" requestid=10
App::Journal BuyAsset UserBacket{"user_id":"Alice","backet":Backet{"asset_id":"milk","count":5,},} requestid=10
        "#;

    #[test]
    fn test_all() {
        let all_parsed = read_log(SOURCE1.as_bytes(), ReadMode::All, &[]).unwrap();
        assert_eq!(all_parsed.len(), 1);

        let all_parsed = read_log(SOURCE.as_bytes(), ReadMode::All, &[]).unwrap();
        println!("all parsed:");
        all_parsed.iter().for_each(|parsed| println!("  {:?}", parsed));
        // 2 для начала и конца строки (чтобы первая и последняя кавычки на отдельных строках были)
        // второе число - число пустых строк, которые оставлены для удобства чтения
        assert_eq!(all_parsed.len(), SOURCE.lines().count() - 2 - 7);
    }

    #[test]
    fn test_errors_mode() {
        let only_errors = read_log(SOURCE.as_bytes(), ReadMode::Errors, &[]).unwrap();
        assert!(only_errors.iter().all(|l| matches!(
            l.kind,
            LogKind::System(SystemLogKind::Error(_)) | LogKind::App(AppLogKind::Error(_))
        )));
    }

    #[test]
    fn test_request_id_filter() {
        let only_3 = read_log(SOURCE.as_bytes(), ReadMode::All, &[3]).unwrap();
        assert!(only_3.iter().all(|l| l.request_id.get() == 3));
        assert!(!only_3.is_empty());
    }
}