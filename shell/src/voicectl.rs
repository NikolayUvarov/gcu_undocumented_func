//! Voice control in the shell (docs/voice V2, issue 079). The `voice` program hears and recognizes; the shell, the
//! user's agent, decides here what a phrase does — the same programs and lifecycle requests a typed command uses — and
//! what to answer. Dangerous actions (stopping or restarting a service, rebooting) wait for a yes or no, by voice or by
//! Enter / Esc. Replies are in the language of the phrase.
use core::fmt::Write;
use mind::util::FixedBuf;

pub type Text = FixedBuf<240>;

/// What the shell does for a phrase, besides answering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Do { Nothing, Run(&'static str), Kill(u64), Stop(&'static str), Restart(&'static str), Reboot, Read(&'static str) }

/// What `voice` does after speaking the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Then { Wait, ListenYesNo }

/// A heard phrase as `voice` reports it (idl/voice.wit `next`).
pub struct Heard<'a> { pub understood: bool, pub text: &'a str, pub intent: &'a str, pub slots: &'a str }

/// The state of the conversation.
#[derive(Default)]
pub struct Dialogue {
    /// The action waiting for yes or no, and whether it was asked in Russian.
    pub pending: Option<(Do, bool)>,
    /// The last answer (for "repeat").
    pub last: Text,
    /// The last program voice control started (for "close").
    pub started: Option<u64>,
}

/// The valid UTF-8 text of a buffer (a cut in the middle of a character is dropped).
pub fn text<const N: usize>(buffer: &FixedBuf<N>) -> &str {
    let bytes = buffer.as_bytes();
    core::str::from_utf8(bytes).unwrap_or_else(|e| core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""))
}

fn russian(text: &str) -> bool { text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) }

/// The value of `name` in `slots` ("tool=fm service=rtc").
fn slot<'a>(slots: &'a str, name: &str) -> Option<&'a str> {
    slots.split_whitespace().find_map(|pair| pair.split_once('=').filter(|(n, _)| *n == name).map(|(_, v)| v))
}

/// The tools voice control may open: program, its name in Russian (after «Запускаю») and in English.
const TOOLS: [(&str, &str, &str); 8] = [
    ("fm", "файловый менеджер", "the file manager"), ("edit", "редактор", "the editor"), ("top", "монитор процессов", "the process monitor"),
    ("memmap", "карту памяти", "the memory map"), ("load", "монитор загрузки", "the load monitor"), ("hw", "сведения об оборудовании", "the hardware report"),
    ("dmesg", "журнал", "the log"), ("svc", "список служб", "the services"),
];
/// The services voice control may stop or restart.
const SERVICES: [&str; 3] = ["rtc", "netstack", "sysmon"];
/// The files it may read aloud.
const FILES: [&str; 1] = ["docs/notes.txt"];

// ---- Numbers in words (the synthesizer reads digits one by one) ----

const RU_UNITS: [&str; 10] = ["ноль", "один", "два", "три", "четыре", "пять", "шесть", "семь", "восемь", "девять"];
const RU_TEENS: [&str; 10] = ["десять", "одиннадцать", "двенадцать", "тринадцать", "четырнадцать", "пятнадцать", "шестнадцать", "семнадцать", "восемнадцать", "девятнадцать"];
const RU_TENS: [&str; 10] = ["", "", "двадцать", "тридцать", "сорок", "пятьдесят", "шестьдесят", "семьдесят", "восемьдесят", "девяносто"];
const RU_HUNDREDS: [&str; 10] = ["", "сто", "двести", "триста", "четыреста", "пятьсот", "шестьсот", "семьсот", "восемьсот", "девятьсот"];
const EN_UNITS: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen",
                              "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
const EN_TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

/// The Russian form for `n` of one, few, many (час, часа, часов).
pub fn ru_form<'a>(n: u32, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    match (n % 10, n % 100) { (1, h) if h != 11 => one, (2..=4, h) if !(12..=14).contains(&h) => few, _ => many }
}

/// `n` (below 10 000) in Russian words; `feminine` for «одна», «две».
pub fn ru_number(n: u32, feminine: bool, out: &mut impl Write) {
    let mut words: [&str; 6] = [""; 6];
    let mut k = 0;
    let mut push = |w: &'static str| { if !w.is_empty() { words[k] = w; k += 1; } };
    if n == 0 { push("ноль"); }
    let (thousands, rest) = (n / 1000 % 10, n % 1000);
    if thousands > 0 {
        push(match thousands { 1 => "одна", 2 => "две", t => RU_UNITS[t as usize] });
        push(ru_form(thousands, "тысяча", "тысячи", "тысяч"));
    }
    push(RU_HUNDREDS[(rest / 100) as usize]);
    let (tens, units) = (rest / 10 % 10, rest % 10);
    if tens == 1 { push(RU_TEENS[units as usize]); } else {
        push(RU_TENS[tens as usize]);
        if units > 0 { push(match (units, feminine) { (1, true) => "одна", (2, true) => "две", (u, _) => RU_UNITS[u as usize] }); }
    }
    for (i, w) in words[..k].iter().enumerate() { let _ = write!(out, "{}{}", if i > 0 { " " } else { "" }, w); }
}

/// `n` (below 10 000) in English words.
pub fn en_number(n: u32, out: &mut impl Write) {
    let (thousands, hundreds, rest) = (n / 1000 % 10, n / 100 % 10, n % 100);
    let mut first = true;
    let mut word = |w: &str, out: &mut dyn Write| { let _ = write!(out, "{}{}", if first { "" } else { " " }, w); first = false; };
    if thousands > 0 { word(EN_UNITS[thousands as usize], out); word("thousand", out); }
    if hundreds > 0 { word(EN_UNITS[hundreds as usize], out); word("hundred", out); }
    if rest > 0 || n == 0 {
        if rest < 20 { word(EN_UNITS[rest as usize], out); } else {
            word(EN_TENS[(rest / 10) as usize], out);
            if rest % 10 > 0 { word(EN_UNITS[(rest % 10) as usize], out); }
        }
    }
}

const RU_ORDINAL: [&str; 20] = ["", "первое", "второе", "третье", "четвёртое", "пятое", "шестое", "седьмое", "восьмое", "девятое", "десятое", "одиннадцатое",
                                "двенадцатое", "тринадцатое", "четырнадцатое", "пятнадцатое", "шестнадцатое", "семнадцатое", "восемнадцатое", "девятнадцатое"];
const EN_ORDINAL: [&str; 20] = ["", "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth", "eleventh", "twelfth",
                                "thirteenth", "fourteenth", "fifteenth", "sixteenth", "seventeenth", "eighteenth", "nineteenth"];
const RU_MONTHS: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
const EN_MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// The day of the month as a Russian neuter ordinal («четвёртое», «двадцать первое»).
pub fn ru_day(day: u32, out: &mut impl Write) {
    let _ = match day { 1..=19 => write!(out, "{}", RU_ORDINAL[day as usize]), 20 => write!(out, "двадцатое"), 30 => write!(out, "тридцатое"),
                        d => write!(out, "{} {}", RU_TENS[(d / 10) as usize], RU_ORDINAL[(d % 10) as usize]) };
}

/// The day of the month as an English ordinal ("fourth", "twenty first").
pub fn en_day(day: u32, out: &mut impl Write) {
    let _ = match day { 1..=19 => write!(out, "{}", EN_ORDINAL[day as usize]), 20 => write!(out, "twentieth"), 30 => write!(out, "thirtieth"),
                        d => write!(out, "{} {}", EN_TENS[(d / 10) as usize], EN_ORDINAL[(d % 10) as usize]) };
}

/// What the shell knows when it answers: the time, the date, free kernel memory.
pub struct Facts { pub seconds: Option<usize>, pub date: Option<(u32, u32, u32)>, pub free_bytes: usize }

impl Dialogue {
    /// Decides what a phrase does: the answer to speak, what to do next and the action for the shell.
    pub fn decide(&mut self, heard: &Heard, facts: &Facts) -> (Text, Then, Do) {
        let ru = russian(heard.text);
        let mut say = Text::new();
        let mut then = Then::Wait;
        let mut action = Do::Nothing;
        let pick = |ru: bool, r: &'static str, e: &'static str| if ru { r } else { e };
        if let Some((pending, asked_ru)) = self.pending.take() {
            // A confirmation was asked: only yes goes ahead.
            if heard.understood && heard.intent == "yes" {
                action = pending;
                let _ = match pending {
                    Do::Stop(name) => write!(say, "{} {}", pick(asked_ru, "Останавливаю службу", "Stopping the service"), name),
                    Do::Restart(name) => write!(say, "{} {}", pick(asked_ru, "Перезапускаю службу", "Restarting the service"), name),
                    Do::Reboot => write!(say, "{}", pick(asked_ru, "Перезагружаю систему", "Restarting the system")),
                    _ => Ok(()),
                };
            } else {
                let _ = write!(say, "{}", pick(asked_ru, "Отменено", "Cancelled"));
            }
            self.last = Text::new();
            let _ = write!(self.last, "{}", text(&say));
            return (say, then, action);
        }
        if !heard.understood {
            let _ = write!(say, "{}", pick(ru, "Не понял", "I did not understand"));
            return (say, then, action);
        }
        match heard.intent {
            "open" => match slot(heard.slots, "tool").and_then(|t| TOOLS.iter().find(|(name, _, _)| *name == t)) {
                Some(&(program, ru_name, en_name)) => { let _ = write!(say, "{} {}", pick(ru, "Запускаю", "Starting"), pick(ru, ru_name, en_name)); action = Do::Run(program); }
                None => { let _ = write!(say, "{}", pick(ru, "Не знаю такой программы", "I do not know that program")); }
            },
            "close" => match self.started.take() {
                Some(pid) => { let _ = write!(say, "{}", pick(ru, "Закрываю", "Closing")); action = Do::Kill(pid); }
                None => { let _ = write!(say, "{}", pick(ru, "Мне нечего закрывать", "There is nothing to close")); }
            },
            "time" => match facts.seconds {
                Some(s) => {
                    let (h, m) = ((s / 3600) as u32, (s / 60 % 60) as u32);
                    if ru {
                        let _ = write!(say, "Сейчас "); ru_number(h, false, &mut say); let _ = write!(say, " {} ", ru_form(h, "час", "часа", "часов"));
                        ru_number(m, true, &mut say); let _ = write!(say, " {}", ru_form(m, "минута", "минуты", "минут"));
                    } else {
                        let _ = write!(say, "It is "); en_number(h, &mut say); let _ = write!(say, " hours "); en_number(m, &mut say); let _ = write!(say, " minutes");
                    }
                }
                None => { let _ = write!(say, "{}", pick(ru, "Часы недоступны", "The clock is not available")); }
            },
            "date" => match facts.date {
                Some((_, month, day)) if (1..=12).contains(&month) && (1..=31).contains(&day) => {
                    if ru { let _ = write!(say, "Сегодня "); ru_day(day, &mut say); let _ = write!(say, " {}", RU_MONTHS[month as usize - 1]); }
                    else { let _ = write!(say, "Today is {} ", EN_MONTHS[month as usize - 1]); en_day(day, &mut say); }
                }
                _ => { let _ = write!(say, "{}", pick(ru, "Часы недоступны", "The clock is not available")); }
            },
            "memory" => {
                let megabytes = (facts.free_bytes >> 20) as u32;
                if ru { let _ = write!(say, "Свободно мегабайт: "); ru_number(megabytes, false, &mut say); }
                else { let _ = write!(say, "Free memory: "); en_number(megabytes, &mut say); let _ = write!(say, " megabytes"); }
            }
            "read" => match slot(heard.slots, "file").and_then(|f| FILES.iter().find(|&&name| name == f)) {
                Some(&file) => action = Do::Read(file),
                None => { let _ = write!(say, "{}", pick(ru, "Не знаю такого файла", "I do not know that file")); }
            },
            "stop" | "restart" => match slot(heard.slots, "service").and_then(|s| SERVICES.iter().find(|&&name| name == s)) {
                Some(&service) => {
                    let stop = heard.intent == "stop";
                    let _ = write!(say, "{} {}?", if stop { pick(ru, "Остановить службу", "Stop the service") } else { pick(ru, "Перезапустить службу", "Restart the service") }, service);
                    self.pending = Some((if stop { Do::Stop(service) } else { Do::Restart(service) }, ru));
                    then = Then::ListenYesNo;
                }
                None => { let _ = write!(say, "{}", pick(ru, "Не знаю такой службы", "I do not know that service")); }
            },
            "reboot" => {
                let _ = write!(say, "{}", pick(ru, "Перезагрузить систему?", "Restart the system?"));
                self.pending = Some((Do::Reboot, ru));
                then = Then::ListenYesNo;
            }
            "help" => { let _ = write!(say, "{}", pick(ru, "Скажите: открой файлы, который час, какое сегодня число, сколько свободной памяти, останови службу",
                                                    "Say: open files, what time is it, what is the date, how much memory is free, stop the service")); }
            "repeat" => { let _ = write!(say, "{}", if self.last.as_bytes().is_empty() { pick(ru, "Мне нечего повторить", "There is nothing to repeat") } else { text(&self.last) }); return (say, then, action); }
            "yes" | "no" => { let _ = write!(say, "{}", pick(ru, "Мне нечего подтверждать", "There is nothing to confirm")); }
            "cancel" => { let _ = write!(say, "{}", pick(ru, "Хорошо", "All right")); }
            _ => { let _ = write!(say, "{}", pick(ru, "Не понял", "I did not understand")); }
        }
        self.last = Text::new();
        let _ = write!(self.last, "{}", text(&say));
        (say, then, action)
    }

    /// The keyboard answers a pending confirmation: Enter is yes, Esc is no. Returns the action for yes.
    pub fn answer_by_key(&mut self, yes: bool) -> Option<Do> { self.pending.take().and_then(|(action, _)| yes.then_some(action)) }
}

// ---- The shell's side: the `voice` program, its calls and push-to-talk ----

use super::{error_text, Shell};
use mind::abi::{CAP_GRANT, CAP_WRITE, KEY_F1, SLOT_INIT};
use mind::control;
use mind::idl::voice::{self as idl, Action, Order};
use mind::idl::{init as idl_init, wire};
use mind::ipc::{Endpoint, Received};

/// Where the buffer of voice's call arrives: a fixed slot the shell does not use (18 is SLOT_NETWORK in applications).
/// A deferred call keeps it until the shell answers.
pub const VOICE_RECEIVE: usize = 18;

/// Voice control in the shell: the endpoint `voice` calls the shell on, the program, its call waiting for the next
/// order (answered at push-to-talk), and the conversation.
#[derive(Default)]
pub struct Voice {
    pub endpoint: Option<Endpoint>,
    pid: Option<u64>,
    call: Option<wire::Call>,
    /// Push-to-talk came while voice was busy (speaking): listen at its next call.
    wanted: bool,
    /// An order to listen is out; `cancelled`: Esc or a key answer came meanwhile, so what voice hears is dropped.
    listening: bool,
    cancelled: bool,
    /// Reboot at voice's next call, once the answer has been spoken.
    reboot: bool,
    /// A line about voice control was printed: the prompt follows.
    noted: bool,
    dialogue: Dialogue,
}

/// `text` cut to fit `N` bytes at a character boundary.
fn fit<const N: usize>(text: &str) -> mind::idl::codec::Text<N> {
    let mut end = text.len().min(N);
    while !text.is_char_boundary(end) { end -= 1; }
    mind::idl::codec::Text::new(&text[..end]).unwrap_or_default()
}

impl Shell {
    fn voice_on(&self) -> bool { self.voice.pid.is_some_and(mind::process::alive) }

    /// Forgets the conversation and a call of a `voice` that is gone (its buffer and receive slot are freed).
    fn voice_reset(&mut self) {
        if let Some(call) = self.voice.call.take() { let _ = idl::reply_next(call, &Order { say: fit(""), action: Action::Quit }); }
        let endpoint = self.voice.endpoint;
        if self.voice.pid.is_some() { let _ = mind::input::listen(KEY_F1 + 11, 0, false); }
        self.voice = Voice { endpoint, ..Voice::default() };
    }

    /// `voice on [--wav FILE] [SECONDS]`, `voice off`, `voice listen`, `voice` (status).
    pub fn voice_command(&mut self, args: &[u8]) {
        let split = args.iter().position(|b| b.is_ascii_whitespace()).unwrap_or(args.len());
        let (word, rest) = (&args[..split], args[split..].trim_ascii());
        let on = self.voice_on();
        match word {
            b"on" => {
                if on { return self.report("VOICE CONTROL IS ALREADY ON"); }
                self.voice_reset();
                let endpoint = match self.voice.endpoint {
                    Some(endpoint) => endpoint,
                    None => match Endpoint::create() { Ok(endpoint) => { self.voice.endpoint = Some(endpoint); endpoint } Err(_) => return self.report("NO ENDPOINT FOR VOICE CONTROL") },
                };
                // voice gets a client of the shell's endpoint in its SLOT_INIT; the loader keeps a copy until it starts.
                let Ok(client) = mind::ipc::mint(endpoint.0, CAP_WRITE | CAP_GRANT, 0, 0) else { return self.report("OUT OF CAPABILITY SLOTS") };
                let started = self.start_with(b"voice", rest, false, &[(SLOT_INIT, client)]);
                let _ = mind::ipc::drop_cap(client);
                match started {
                    Ok(pid) => {
                        self.voice.pid = Some(pid);
                        // F12 reaches the shell whatever program has the focus (issue 154).
                        let _ = mind::input::listen(KEY_F1 + 11, 0, true);
                        self.started(pid, b"voice", true);
                        let _ = writeln!(self.term, "VOICE CONTROL ON. F12 OR VOICE LISTEN: SAY A COMMAND, ESC: CANCEL. VOICE OFF: STOP.");
                    }
                    Err(error) => self.report(error_text(error, false)),
                }
            }
            b"off" if rest.is_empty() => {
                if !on { return self.report("VOICE CONTROL IS OFF"); }
                // At once if voice waits for an order; otherwise it is stopped where it is (speaking or listening).
                match self.voice.call.take() {
                    Some(call) => { let _ = idl::reply_next(call, &Order { say: fit(""), action: Action::Quit }); }
                    None => if let Some(pid) = self.voice.pid { let _ = control::kill(pid); },
                }
                self.voice_reset();
                let _ = writeln!(self.term, "VOICE CONTROL OFF");
            }
            b"listen" if rest.is_empty() => match self.voice_listen() {
                Ok(()) => { let _ = writeln!(self.term, "VOICE: LISTENING"); }
                Err(error) => self.report(error),
            },
            b"" => match self.voice.pid.filter(|_| on) {
                Some(pid) => { let _ = writeln!(self.term, "VOICE CONTROL ON PID={}{}", pid, if self.voice.listening { " LISTENING" } else { "" }); }
                None => { let _ = writeln!(self.term, "VOICE CONTROL OFF"); }
            },
            _ => self.report("USAGE: VOICE ON [--wav FILE] [SECONDS] | VOICE OFF | VOICE LISTEN"),
        }
    }

    /// Push-to-talk: voice listens for one utterance (now, or after what it is saying).
    fn voice_listen(&mut self) -> Result<(), &'static str> {
        if !self.voice_on() { return Err("VOICE CONTROL IS OFF (VOICE ON)"); }
        if self.voice.listening && !self.voice.cancelled { return Err("VOICE IS ALREADY LISTENING"); }
        match self.voice.call.take() {
            Some(call) => self.voice_order(call, "", Action::Listen),
            None => self.voice.wanted = true,
        }
        Ok(())
    }

    /// A key for voice control while the shell has the focus: F12 is push-to-talk; Esc cancels listening and, like
    /// Enter on an empty line, answers a pending confirmation. Returns whether the key was taken.
    pub fn voice_key(&mut self, key: mind::input::Key) -> bool {
        use mind::input::Code;
        if key.code() == Code::F(12) {
            let note = match self.voice_listen() { Ok(()) => "VOICE: LISTENING", Err(error) => error };
            self.voice_note(format_args!("{}", note));
            self.voice_done();
            return true;
        }
        let enter = key.code() == Code::Enter && self.line.is_empty();
        if !(key.is_escape() || enter) || !self.voice_on() || !(self.voice.listening || self.voice.dialogue.pending.is_some()) { return false; }
        // What voice hears for this question no longer counts.
        if self.voice.listening { self.voice.cancelled = true; }
        match self.voice.dialogue.answer_by_key(enter) {
            Some(action) => { let mut say = Text::new(); self.voice_do(action, false, &mut say); self.voice_note(format_args!("VOICE: CONFIRMED BY KEY")); }
            None => self.voice_note(format_args!("VOICE: CANCELLED")),
        }
        self.voice_done();
        true
    }

    /// A line about voice control between commands (`voice_done` shows the prompt and the typed line again).
    fn voice_note(&mut self, args: core::fmt::Arguments) {
        if self.term.position().col != 0 { self.term.print_char(b'\n'); }
        let _ = self.term.write_fmt(args);
        self.term.print_char(b'\n');
        self.voice.noted = true;
    }

    fn voice_done(&mut self) {
        if core::mem::take(&mut self.voice.noted) && self.focused.is_none() && self.console.is_none() { self.prompt(); }
    }

    /// Answers voice's call: speak `say`, then `action`.
    fn voice_order(&mut self, call: wire::Call, say: &str, action: Action) {
        self.voice.listening = matches!(action, Action::Listen | Action::ListenYesNo);
        self.voice.cancelled = false;
        if idl::reply_next(call, &Order { say: fit(say), action }).is_err() { self.voice.listening = false; }
    }

    /// Does what a phrase asked for; `ru` is the language of the answer it may add to `say`.
    fn voice_do(&mut self, action: Do, ru: bool, say: &mut Text) {
        let service_error = |result: mind::sys::Result<Result<(), idl_init::Error>>| match result { Ok(Ok(())) => None, Ok(Err(error)) => Some(error), Err(_) => Some(idl_init::Error::Failed) };
        match action {
            Do::Nothing => {}
            Do::Run(program) => match self.start(program.as_bytes(), b"", false) {
                Ok(pid) => { self.voice.dialogue.started = Some(pid); self.voice_note(format_args!("VOICE: RUN {}", program)); self.started(pid, program.as_bytes(), false); }
                Err(error) => self.voice_note(format_args!("VOICE: RUN {}: {}", program, error_text(error, false))),
            },
            Do::Kill(pid) => match control::kill(pid) {
                Ok(()) => self.voice_note(format_args!("VOICE: KILLED PID={}", pid)),
                Err(_) => self.voice_note(format_args!("VOICE: PID={} HAS ENDED", pid)),
            },
            Do::Stop(name) | Do::Restart(name) => {
                let stop = matches!(action, Do::Stop(_));
                let failed = if stop { service_error(idl_init::stop(Endpoint::INIT, name)) } else { service_error(idl_init::restart(Endpoint::INIT, name).map(|r| r.map(drop))) };
                match failed {
                    None => self.voice_note(format_args!("VOICE: {} {}", if stop { "STOPPED" } else { "RESTARTED" }, name)),
                    Some(error) => {
                        self.voice_note(format_args!("VOICE: {} {}: {:?}", if stop { "STOP" } else { "RESTART" }, name, error));
                        *say = Text::new();
                        let _ = write!(say, "{}", if ru { "Не получилось" } else { "It did not work" });
                    }
                }
            }
            Do::Reboot => self.voice.reboot = true,
            Do::Read(path) => {
                // The first lines of the file, up to what one answer holds.
                let mut bytes = [0u8; 236];
                let read = mind::fs::File::open(path).and_then(|mut file| file.read(&mut bytes));
                *say = Text::new();
                match read {
                    Ok(length) => {
                        let text = match core::str::from_utf8(&bytes[..length]) { Ok(text) => text, Err(e) => core::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or("") };
                        // Whole lines only, when there is more than one.
                        let text = if length == bytes.len() { text.rfind('\n').map_or(text, |end| &text[..end]) } else { text };
                        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                            let end = if line.ends_with(['.', '!', '?']) { "" } else { "." };
                            let _ = write!(say, "{}{}{}", if say.as_bytes().is_empty() { "" } else { " " }, line, end);
                        }
                        self.voice_note(format_args!("VOICE: READ {}", path));
                    }
                    Err(error) => {
                        let _ = write!(say, "{}", if ru { "Не могу прочитать файл" } else { "I cannot read the file" });
                        self.voice_note(format_args!("VOICE: READ {}: {:?}", path, error));
                    }
                }
            }
        }
    }

    /// The `voice` program gone (exited or killed): its call and the conversation are dropped.
    pub fn voice_check(&mut self) {
        if let Some(pid) = self.voice.pid.filter(|&pid| !mind::process::alive(pid)) {
            self.voice_reset();
            self.voice_note(format_args!("VOICE CONTROL OFF (PID={} ENDED)", pid));
            self.voice_done();
        }
    }

    /// voice's call (idl/voice.wit `next`): what it heard, if it was told to listen; the answer is the next order. With
    /// nothing to say the call waits for push-to-talk.
    pub fn voice_message(&mut self, request: &Received) {
        self.voice_answer(request);
        self.voice_done();
    }

    fn voice_answer(&mut self, request: &Received) {
        let (idl::Request::Next { heard, understood, text: phrase, intent, slots, confidence }, call) = match idl::decode(request, VOICE_RECEIVE) {
            Ok(decoded) => decoded,
            Err(reason) => { let _ = wire::reject(reason); return; }
        };
        // Only the voice program this shell started holds a client.
        if self.voice.pid != Some(request.sender) { let _ = idl::reply_next(call, &Order { say: fit(""), action: Action::Quit }); return; }
        if core::mem::take(&mut self.voice.reboot) {
            // The answer has been spoken. If the reboot is refused, voice waits for the next push-to-talk.
            let mut call = call;
            if call.defer().is_ok() { self.voice.call = Some(call); }
            self.voice_note(format_args!("VOICE: REBOOT"));
            return super::power::reboot(&mut self.term, b"");
        }
        let listened = core::mem::take(&mut self.voice.listening) && !core::mem::take(&mut self.voice.cancelled);
        if listened && (heard || self.voice.dialogue.pending.is_some()) {
            let heard = Heard { understood, text: phrase.as_str(), intent: intent.as_str(), slots: slots.as_str() };
            let facts = Facts { seconds: mind::rtc::seconds_since_midnight(), date: mind::rtc::date(), free_bytes: control::kernel_heap().1 };
            let (mut say, then, action) = self.voice.dialogue.decide(&heard, &facts);
            let ru = russian(phrase.as_str()) || (phrase.as_str().is_empty() && russian(text(&say)));
            match (heard.understood, phrase.as_str().is_empty()) {
                (_, true) => self.voice_note(format_args!("VOICE: NOTHING HEARD -> {}", text(&say))),
                (true, false) => self.voice_note(format_args!("VOICE: \"{}\" ({}) -> {}", phrase.as_str(), confidence, text(&say))),
                (false, false) => self.voice_note(format_args!("VOICE: NOT UNDERSTOOD (CLOSEST \"{}\") -> {}", phrase.as_str(), text(&say))),
            }
            self.voice_do(action, ru, &mut say);
            let action = match then { Then::Wait => Action::Wait, Then::ListenYesNo => Action::ListenYesNo };
            let mut copy = Text::new();
            let _ = write!(copy, "{}", text(&say));
            return self.voice_order(call, text(&copy), action);
        }
        if listened { self.voice_note(format_args!("VOICE: NOTHING HEARD")); }
        if core::mem::take(&mut self.voice.wanted) { return self.voice_order(call, "", Action::Listen); }
        let mut call = call;
        if call.defer().is_ok() { self.voice.call = Some(call); }
    }
}
