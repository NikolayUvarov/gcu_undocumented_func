//! A request that asks for a credential, a key or a change of authority (docs/hub/README.md, 6.2) is flagged to the
//! admin. Such a request waits for the admin as a held one does, and no agent reads it until the admin clears it. This
//! is a list of words, not an understanding: it flags what it names and misses what it does not. The agents still read
//! every request as data (MC-11.5).

/// The words of each reason, in English and Russian, matched in the request's text lowered and with its spaces
/// collapsed; a word list's entry matches anywhere, so `password` matches `passwords` too.
const REASONS: &[(&str, &[&str])] = &[
    ("credential", &[
        "password", "passphrase", "passcode", "credential", "api key", "api-key", "apikey", "access token", "auth token", "bearer token",
        "private key", "secret key", "signing key", "ssh key", "ssh-key", "seed phrase", "recovery key", "recovery code", "2fa code", "one-time code",
        "парол", "учётные данные", "учетные данные", "логин и пароль", "токен", "приватный ключ", "закрытый ключ", "секретный ключ",
        "ключ подписи", "ssh-ключ", "ключ восстановления", "сид-фраз", "код подтверждения",
    ]),
    ("authority", &[
        "root access", "admin access", "admin rights", "administrator rights", "make me admin", "make me an admin", "make me the admin",
        "give me access", "grant me access", "sudo", "maintainer rights", "commit access", "push access", "write access to the repo",
        "права администратора", "права админа", "сделай меня админ", "сделайте меня админ", "дай мне доступ", "дайте мне доступ", "дайте доступ",
        "root-доступ", "рут-доступ", "доступ к серверу", "доступ к репозиторию", "права на запись", "повысь мои права", "повысьте мои права",
    ]),
];

/// The reasons a request's title and text give to flag it, each once, in the list's order.
pub fn reasons(title: &str, text: &str) -> Vec<&'static str> {
    let lowered = format!("{title} {text}").to_lowercase();
    let collapsed = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    REASONS.iter().filter(|(_, words)| words.iter().any(|w| collapsed.contains(w))).map(|(reason, _)| *reason).collect()
}
