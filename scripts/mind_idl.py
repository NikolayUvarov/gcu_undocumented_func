#!/usr/bin/env python3
"""MIND IDL v0.2: generates libmind bindings from a WIT subset (docs/idl/README.md).

Usage: mind_idl.py            write libmind/src/idl/<interface>.rs for every idl/*.wit
       mind_idl.py --check    fail if a generated file is missing or out of date
       mind_idl.py --one SOURCE TARGET   generate one interface into TARGET (the generator's host tests)
       --root DIR             use DIR instead of the repository root
"""
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IDL_DIR = ROOT / "idl"
OUT_DIR = ROOT / "libmind" / "src" / "idl"

INTEGERS = {"bool": 1, "u8": 8, "u16": 16, "u32": 32, "u64": 64}
HANDLES = {"memory": "CAP_KIND_MEMORY", "endpoint": "CAP_KIND_ENDPOINT"}
WORD0_START, WORD_BITS = 16, 64  # word calls: method:8 | major:8 | fields
BUFFER_MAX = 64 * 1024  # largest request or reply of a buffer call
PAGE = 4096
# Names the generated code uses itself; an interface type may not take them (`error` is allowed: the system error
# type is then imported as `SysError`).
RESERVED = {"Result", "Endpoint", "Received", "Pages", "Wire", "Reader", "Writer", "Text", "List", "Call", "Reject", "Request", "SysError"}


class IdlError(Exception):
    pass


# Types: ("int", name) · ("enum", name) · ("str", n) · ("bytes", n) · ("list", type, n) · ("rec", name) ·
# ("handle", mode, kind). `records` holds the records and enums declared so far.
def parse_type(text, records, where):
    text = text.strip()
    if text in INTEGERS:
        return ("int", text)
    if m := re.fullmatch(r"string<\s*(\d+)\s*>", text):
        return ("str", int(m.group(1)))
    if m := re.fullmatch(r"bytes<\s*(\d+)\s*>", text):
        if not 1 <= int(m.group(1)) <= 0xFFFF:
            raise IdlError(f"{where}: bytes<N> needs 1 <= N <= 65535")
        return ("bytes", int(m.group(1)))
    if m := re.fullmatch(r"list<\s*(.+)\s*,\s*(\d+)\s*>", text):
        inner = parse_type(m.group(1), records, where)
        if inner[0] not in ("int", "enum", "str", "rec"):
            raise IdlError(f"{where}: list items must be integers, enums, strings or records")
        return ("list", inner, int(m.group(2)))
    if m := re.fullmatch(r"(own|borrow)<\s*(\w+)\s*>", text):
        if m.group(2) not in HANDLES:
            raise IdlError(f"{where}: unknown handle kind '{m.group(2)}'")
        return ("handle", m.group(1), m.group(2))
    if text in records:
        return ("enum", text) if isinstance(records[text], Enum) else ("rec", text)
    raise IdlError(f"{where}: unsupported type '{text}'")


def bounded(t):
    return t[0] in ("str", "list", "rec", "bytes")


def max_size(t, records):
    kind = t[0]
    if kind == "int":
        return max(1, INTEGERS[t[1]] // 8)
    if kind == "enum":
        return 1
    if kind in ("str", "bytes"):
        return 2 + t[1]
    if kind == "list":
        return 2 + t[2] * max_size(t[1], records)
    if kind == "rec":
        return sum(max_size(f.type, records) for f in records[t[1]].fields)
    raise IdlError(f"type {t} has no wire size")


@dataclass
class Field:
    name: str
    type: tuple
    word: int = 0
    shift: int = 0

    @property
    def bits(self):
        return 8 if self.type[0] == "enum" else INTEGERS[self.type[1]]


@dataclass
class Record:
    name: str
    doc: list
    fields: list


@dataclass
class Enum:
    name: str
    doc: list
    cases: list


@dataclass
class Function:
    name: str
    doc: list
    params: list
    result: tuple | None  # (type or None, optional, fallible, enum error name or None)
    index: int = 0
    buffered: bool = False
    handle: Field | None = None
    used: list = field(default_factory=lambda: [0, 0])
    result_field: Field | None = None
    request_max: int = 0
    reply_max: int = 0


@dataclass
class Interface:
    package: str
    name: str
    version: tuple
    doc: list
    records: dict
    functions: list


def snake(name):
    return name.replace("-", "_")


def camel(name):
    return "".join(part.capitalize() for part in re.split(r"[-_]", name))


def layout(fields, start=WORD0_START):
    """Places integer fields of a word call in declaration order; a field never straddles words."""
    used, word, offset = [0, 0], 0, start
    for f in fields:
        if offset + f.bits > WORD_BITS:
            word, offset = word + 1, 0
        if word > 1:
            raise IdlError(f"fields do not fit in two message words at '{f.name}'")
        f.word, f.shift = word, offset
        used[word] |= ((1 << f.bits) - 1) << offset
        offset += f.bits
    return used


def parse_result(text, records, where):
    if text is None:
        return None
    text = text.strip()
    if m := re.fullmatch(r"option<(.+)>", text):
        inner = parse_type(m.group(1), records, where)
        if inner[0] == "handle":
            raise IdlError(f"{where}: unsupported result type '{text}'")
        return (inner, True, False, None)
    if m := re.fullmatch(r"result<\s*(.+?)\s*,\s*([\w-]+)\s*>", text):
        error = m.group(2)
        if error != "error-code" and not isinstance(records.get(error), Enum):
            raise IdlError(f"{where}: the error of a result is error-code or an enum, not '{error}'")
        inner = None if m.group(1) == "_" else parse_type(m.group(1), records, where)
        return (inner, False, error == "error-code", None if error == "error-code" else error)
    return (parse_type(text, records, where), False, False, None)


def parse(text, source="<idl>"):
    package = name = version = None
    doc, pending, functions, records, depth = [], [], [], {}, 0
    record = enum = None
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        where = f"{source}:{number}"
        if not line:
            continue
        if line.startswith("///"):
            pending.append(line[3:].strip())
            continue
        if line.startswith("//"):
            continue
        if enum is not None:
            if line == "}":
                declare(records, enum, where)
                enum = None
                continue
            enum.cases += [c.strip() for c in line.split(",") if c.strip()]
            pending = []
            continue
        if record is not None:
            if line == "}":
                if not record.fields:
                    raise IdlError(f"{where}: empty record '{record.name}'")
                declare(records, record, where)
                record = None
                continue
            for part in split_params(line):  # one or several fields per line
                m = re.fullmatch(r"([\w-]+)\s*:\s*(.+)", part)
                if not m:
                    raise IdlError(f"{where}: bad record field: {line}")
                record.fields.append(Field(snake(m.group(1)), record_field(m.group(2), records, where)))
            pending = []
            continue
        if m := re.fullmatch(r"package\s+([\w-]+:[\w-]+)@(\d+)\.(\d+)\.(\d+);", line):
            package, version = m.group(1), tuple(int(m.group(i)) for i in (2, 3, 4))
            doc, pending = doc + pending, []
            continue
        if m := re.fullmatch(r"interface\s+([\w-]+)\s*\{", line):
            if name or depth:
                raise IdlError(f"{where}: one interface per file")
            name, depth = m.group(1), 1
            doc, pending = doc + pending, []
            continue
        if m := re.fullmatch(r"enum\s+([\w-]+)\s*\{(.*?)(\})?", line):
            if depth != 1:
                raise IdlError(f"{where}: enum outside the interface")
            enum, pending = Enum(m.group(1), pending, [c.strip() for c in m.group(2).split(",") if c.strip()]), []
            if m.group(3):
                declare(records, enum, where)
                enum = None
            continue
        if m := re.fullmatch(r"record\s+([\w-]+)\s*\{(.+)\}", line):  # one-line record
            if depth != 1:
                raise IdlError(f"{where}: record outside the interface")
            fields = []
            for part in split_params(m.group(2)):
                fm = re.fullmatch(r"([\w-]+)\s*:\s*(.+)", part)
                if not fm:
                    raise IdlError(f"{where}: bad record field: {part}")
                fields.append(Field(snake(fm.group(1)), record_field(fm.group(2), records, where)))
            declare(records, Record(m.group(1), pending, fields), where)
            pending = []
            continue
        if m := re.fullmatch(r"record\s+([\w-]+)\s*\{", line):
            if depth != 1:
                raise IdlError(f"{where}: record outside the interface")
            record, pending = Record(m.group(1), pending, []), []
            continue
        if line == "}":
            depth -= 1
            continue
        if m := re.fullmatch(r"([\w-]+)\s*:\s*func\((.*)\)\s*(?:->\s*(.+?))?\s*;", line):
            if depth != 1:
                raise IdlError(f"{where}: function outside the interface")
            params = []
            for part in split_params(m.group(2)):
                pm = re.fullmatch(r"([\w-]+)\s*:\s*(.+)", part)
                if not pm:
                    raise IdlError(f"{where}: bad parameter '{part}'")
                params.append(Field(snake(pm.group(1)), parse_type(pm.group(2), records, where)))
            functions.append(Function(snake(m.group(1)), pending, params, parse_result(m.group(3), records, where)))
            pending = []
            continue
        raise IdlError(f"{where}: unsupported syntax: {line}")
    if not package or not name or depth != 0 or record is not None or enum is not None:
        raise IdlError(f"{source}: needs a package line and one closed interface")
    if version[0] == 0 or version[0] > 255:
        raise IdlError(f"{source}: major version must be 1..255")
    if not 1 <= len(functions) <= 255:
        raise IdlError(f"{source}: 1..255 functions")
    for index, function in enumerate(functions, 1):
        check(function, records, f"{source}: {function.name}")
        function.index = index
    return Interface(package, name, version, doc, records, functions)


def record_field(text, records, where):
    t = parse_type(text, records, where)
    if t[0] == "handle":
        raise IdlError(f"{where}: records cannot hold capabilities")
    if t[0] == "bytes":
        raise IdlError(f"{where}: bytes<N> is a parameter or a result, not a record field")
    return t


def declare(records, item, where):
    if item.name in records:
        raise IdlError(f"{where}: '{item.name}' is declared twice")
    if camel(item.name) in RESERVED:
        raise IdlError(f"{where}: the name '{item.name}' is reserved")
    if isinstance(item, Enum):
        if not 1 <= len(item.cases) <= 256 or len(set(item.cases)) != len(item.cases):
            raise IdlError(f"{where}: an enum has 1..256 distinct cases")
        for case in item.cases:
            if not re.fullmatch(r"[a-z][\w-]*", case):
                raise IdlError(f"{where}: bad enum case '{case}'")
    records[item.name] = item


def split_params(text):
    """Splits on commas outside angle brackets."""
    parts, depth, current = [], 0, ""
    for c in text:
        if c == "<":
            depth += 1
        elif c == ">":
            depth -= 1
        if c == "," and depth == 0:
            parts.append(current.strip())
            current = ""
        else:
            current += c
    if current.strip():
        parts.append(current.strip())
    return parts


# Parameter names the generated code uses for its own locals.
BUFFER_LOCALS = {"endpoint", "buffer", "reply", "w", "r", "call", "copy", "scratch", "request", "cap", "out"}
WORD_LOCALS = {"endpoint", "words", "reply", "received", "receive", "out", "none"}


def check(function, records, where):
    result_type = function.result[0] if function.result else None
    for p in function.params:
        if p.name in WORD_LOCALS or (p.type[0] == "enum" and p.name in ("request", "cap")):
            raise IdlError(f"{where}: parameter name '{p.name}' is used by the generated code")
    function.buffered = any(bounded(p.type) for p in function.params) or (result_type is not None and bounded(result_type))
    handles = [p for p in function.params if p.type[0] == "handle"]
    if len(handles) > 1:
        raise IdlError(f"{where}: at most one capability per message")
    if function.buffered:
        for p in function.params:
            if p.name in BUFFER_LOCALS:
                raise IdlError(f"{where}: parameter name '{p.name}' is used by the generated code")
        if handles:
            raise IdlError(f"{where}: a buffer call carries its buffer as the capability; no other capability")
        if result_type is not None and result_type[0] == "handle":
            raise IdlError(f"{where}: a capability result needs a word call")
        function.request_max = sum(max_size(p.type, records) for p in function.params)
        function.reply_max = max_size(result_type, records) if result_type else 0
        if max(function.request_max, function.reply_max) > BUFFER_MAX:
            raise IdlError(f"{where}: more than {BUFFER_MAX} bytes")
        return
    function.handle = handles[0] if handles else None
    function.used = layout([p for p in function.params if p.type[0] in ("int", "enum")])
    if result_type is not None and result_type[0] != "handle":
        function.result_field = Field("value", result_type)
        layout([function.result_field])


# Rust names of types; SYS is the name of the system error type in the file being generated.
SYS = "Error"


def rust(t):
    kind = t[0]
    if kind == "int":
        return t[1]
    if kind == "str":
        return f"Text<{t[1]}>"
    if kind == "bytes":
        return "&'a [u8]"
    if kind == "list":
        return f"List<{rust(t[1])}, {t[2]}>"
    if kind in ("rec", "enum"):
        return camel(t[1])
    return "usize"


def rust_param(t):
    kind = t[0]
    if kind == "str":
        return "&str"
    if kind == "bytes":
        return "&[u8]"
    if kind == "list":
        return f"&[{rust(t[1])}]"
    if kind == "rec":
        return f"&{camel(t[1])}"
    return rust(t)


def encode_expr(t, value, writer="w"):
    kind = t[0]
    if kind == "str":
        return f"codec::encode_str::<{t[1]}>({value}, {writer})"
    if kind == "bytes":
        return f"codec::encode_bytes::<{t[1]}>({value}, {writer})"
    if kind == "list":
        return f"codec::encode_slice::<{rust(t[1])}, {t[2]}>({value}, {writer})"
    return f"{value}.encode({writer})"


def result_rust(function):
    if function.result is None:
        return "()"
    inner, optional, fallible, error = function.result
    if inner is None or inner[0] == "handle":
        value = "()"
    elif inner[0] == "bytes":
        value = "usize"
    else:
        value = rust(inner)
    value = f"Option<{value}>" if optional else value
    return f"core::result::Result<{value}, {camel(error)}>" if error else value


def word_encode(f, expr):
    return f"(({expr}) as usize) << {f.shift}"


def word_decode(f, words):
    value = f"wire::field(&{words}, {f.word}, {f.shift}, {f.bits})"
    if f.type[0] == "enum":
        return f"{camel(f.type[1])}::from_code({value})"
    return f"{value} != 0" if f.type[1] == "bool" else f"{value} as {f.type[1]}"


def generate(interface, source):
    global SYS
    SYS = "SysError" if "Error" in (camel(n) for n in interface.records) else "Error"
    out = []
    w = out.append
    major, minor, patch = interface.version
    w(f"// Generated by scripts/mind_idl.py from {source}; do not edit.")
    for line in interface.doc:
        w(f"//! {line}" if line else "//!")
    w("#![allow(clippy::all, unused_imports, unused_mut, unused_variables)]")
    w("use crate::abi::*;")
    w("use crate::ipc::{Endpoint, Received};")
    w("use crate::mem::Pages;")
    w("use crate::sys::{Error, Result};" if SYS == "Error" else "use crate::sys::{Error as SysError, Result};")
    w("use super::codec::{self, List, Reader, Text, Wire, Writer};")
    w("use super::wire::{self, Call, Reject};")
    w("")
    w(f'pub const PACKAGE: &str = "{interface.package}";')
    w(f"pub const VERSION: (u8, u8, u8) = ({major}, {minor}, {patch});")
    w(f"const MAJOR: usize = {major};")
    w("")
    for item in interface.records.values():
        if isinstance(item, Enum):
            generate_enum(w, item)
            continue
        record = item
        for line in record.doc:
            w(f"/// {line}")
        w("#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]")
        w(f"pub struct {camel(record.name)} {{ {', '.join(f'pub {f.name}: {rust(f.type)}' for f in record.fields)} }}")
        w(f"impl Wire for {camel(record.name)} {{")
        w(f"    const MAX: usize = {' + '.join(f'<{rust(f.type)} as Wire>::MAX' for f in record.fields)};")
        w(f"    fn encode(&self, w: &mut Writer) -> Option<()> {{ {' '.join(f'self.{f.name}.encode(w)?;' for f in record.fields)} Some(()) }}")
        w(f"    fn decode(r: &mut Reader) -> Option<Self> {{ Some(Self {{ {', '.join(f'{f.name}: Wire::decode(r)?' for f in record.fields)} }}) }}")
        w("}")
        w("")
    for f in interface.functions:
        generate_client(w, f)
    generate_server(w, interface)
    return "\n".join(out)


def generate_enum(w, enum):
    name = camel(enum.name)
    for line in enum.doc:
        w(f"/// {line}")
    w("#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]")
    w("#[repr(u8)]")
    w(f"pub enum {name} {{ {', '.join(('#[default] ' if i == 0 else '') + f'{camel(c)} = {i}' for i, c in enumerate(enum.cases))} }}")
    w(f"impl {name} {{")
    w("    /// The case with wire code `code`; None for a code the interface does not define.")
    w(f"    pub fn from_code(code: usize) -> Option<Self> {{ match code {{ {' '.join(f'{i} => Some(Self::{camel(c)}),' for i, c in enumerate(enum.cases))} _ => None }} }}")
    w("}")
    w(f"impl Wire for {name} {{")
    w("    const MAX: usize = 1;")
    w("    fn encode(&self, w: &mut Writer) -> Option<()> { (*self as u8).encode(w) }")
    w("    fn decode(r: &mut Reader) -> Option<Self> { Self::from_code(u8::decode(r)? as usize) }")
    w("}")
    w("")


def generate_client(w, f):
    for line in f.doc:
        w(f"/// {line}")
    inner, optional, fallible, error = f.result if f.result else (None, False, False, None)
    args = ["endpoint: Endpoint"] + [f"{p.name}: {'usize' if p.type[0] == 'handle' else rust_param(p.type)}" for p in f.params]
    if inner is not None and inner[0] == "handle":
        args.append("receive: usize")
    if inner is not None and inner[0] == "bytes":
        args.append("out: &mut [u8]")
    w(f"pub fn {f.name}({', '.join(args)}) -> Result<{result_rust(f)}> {{")
    ok = (lambda value: f"Ok(Ok({value}))") if error else (lambda value: f"Ok({value})")
    enum_error = f"    if let Some(code) = wire::enum_error(&reply)? {{ return Ok(Err({camel(error)}::from_code(code).ok_or({SYS}::Invalid)?)); }}" if error else None
    if f.buffered:
        size = (max(f.request_max, f.reply_max, 1) + PAGE - 1) // PAGE * PAGE
        w(f"    let mut buffer = Pages::new({size}).ok_or({SYS}::NoMemory)?;")
        w("    let length = {")
        w("        let mut w = Writer::new(buffer.as_mut_slice());")
        for p in f.params:
            w(f"        {encode_expr(p.type, p.name, '&mut w')}.ok_or({SYS}::Invalid)?;")
        w("        w.len()")
        w("    };")
        w(f"    let reply = wire::call_buffer(endpoint, {f.index} | MAJOR << 8, &buffer, length)?;")
        if enum_error:
            w(enum_error)
        w(f"    let length = wire::buffer_reply(&reply, {f.reply_max}, {str(optional).lower()}, {str(fallible).lower()})?;")
        if inner is None:
            w(f"    if length != Some(0) {{ return Err({SYS}::Invalid); }}")
            w(f"    {ok('()')}")
        else:
            if inner[0] == "bytes":
                decode = (f"{{ let mut r = Reader::new(&buffer.as_slice()[..length]); let data = codec::decode_bytes::<{inner[1]}>(&mut r).filter(|_| r.done()).ok_or({SYS}::Invalid)?; "
                          f"out.get_mut(..data.len()).ok_or({SYS}::Invalid)?.copy_from_slice(data); data.len() }}")
            else:
                decode = f"{{ let mut r = Reader::new(&buffer.as_slice()[..length]); <{rust(inner)} as Wire>::decode(&mut r).filter(|_| r.done()).ok_or({SYS}::Invalid)? }}"
            if optional:
                w(f"    {ok(f'match length {{ None => None, Some(length) => Some({decode}) }}')}")
            else:
                w(f"    let length = length.ok_or({SYS}::Invalid)?;")
                w(f"    {ok(decode)}")
        w("}")
        w("")
        return
    words = [[f"{f.index}", "MAJOR << 8"], []]
    for p in f.params:
        if p.type[0] in ("int", "enum"):
            words[p.word].append(word_encode(p, p.name))
    w(f"    let words = [{' | '.join(words[0])}, {' | '.join(words[1]) or '0'}];")
    cap = f"Some(({f.handle.name}, {'true' if f.handle.type[1] == 'own' else 'false'}))" if f.handle else "None"
    if inner is not None and inner[0] == "handle":
        # The capability lands in `receive`; one that comes with an error reply is dropped.
        w(f"    let (reply, received) = wire::call_receiving(endpoint, words, {cap}, receive)?;")
        w("    match wire::check_cap_reply(&reply, received, receive)? {")
        w(f"        None => {ok('()')},")
        if error:
            w(f"        Some(code) => Ok(Err({camel(error)}::from_code(wire::enum_code(code)?).ok_or({SYS}::Invalid)?)),")
        elif fallible:
            w(f"        Some(code) => Err(crate::sys::check(code).err().unwrap_or({SYS}::Invalid)),")
        else:
            w(f"        Some(_) => Err({SYS}::Invalid),")
        w("    }")
        w("}")
        w("")
        return
    w(f"    let reply = wire::call(endpoint, words, {cap})?;")
    if fallible:
        w("    wire::check_error(&reply)?;")
    if enum_error:
        w(enum_error)
    if inner is None:
        w(f"    wire::check_reply(&reply, [0, 0], false)?;" if error else "    wire::check_reply(&reply, [0, 0], false).map(drop)")
        if error:
            w(f"    {ok('()')}")
    else:
        rf = f.result_field
        used = [0, 0]
        used[rf.word] = ((1 << rf.bits) - 1) << rf.shift
        w(f"    let {'none' if optional else '_'} = wire::check_reply(&reply, [{used[0]:#x}, {used[1]:#x}], {'true' if optional else 'false'})?;")
        value = word_decode(rf, "reply")
        if inner[0] == "enum":
            value = f"{value}.ok_or({SYS}::Invalid)?"
        w(f"    {ok('if none { None } else { Some(' + value + ') }' if optional else value)}")
    w("}")
    w("")


def generate_server(w, interface):
    # Interfaces with a bytes parameter decode into a caller's scratch buffer: the bytes are borrowed from it.
    borrowed = any(p.type[0] == "bytes" for f in interface.functions for p in f.params)
    life = "<'a>" if borrowed else ""
    if borrowed:
        w("/// Size of the scratch buffer `decode` copies a request into (the largest request).")
        w(f"pub const REQUEST_MAX: usize = {max([f.request_max for f in interface.functions] + [1])};")
        w("")
    w(f"/// A request to the `{interface.name}` interface that passed the receiver's schema check.")
    w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    w(f"pub enum Request{life} {{")
    for f in interface.functions:
        fields = [f"{p.name}: {'usize' if p.type[0] == 'handle' else rust(p.type)}" for p in f.params]
        w(f"    {camel(f.name)}" + (f" {{ {', '.join(fields)} }}," if fields else ","))
    w("}")
    w("")
    w("/// Checks a received message against the schema (MC-2.4): method, major version, unused bits, capability kind, and")
    w("/// for buffer calls every length and value of a private copy of the request (MC-2.11). `cap` is the slot passed to")
    w("/// `recv`; an unexpected capability is dropped (MC-2.12). The `Call` is what the reply functions need.")
    if borrowed:
        w("/// The private copy is `scratch`, which the `bytes` of the request borrow.")
        w("pub fn decode<'a>(request: &Received, cap: usize, scratch: &'a mut [u8; REQUEST_MAX]) -> core::result::Result<(Request<'a>, Call), Reject> {")
    else:
        w("pub fn decode(request: &Received, cap: usize) -> core::result::Result<(Request, Call), Reject> {")
    w("    let words = request.data;")
    w("    wire::header(request, cap, MAJOR)?;")
    w("    match words[0] & 0xFF {")
    for f in interface.functions:
        w(f"        {f.index} => {{")
        if f.buffered:
            if borrowed:
                w(f"            let (call, length) = wire::take_buffer(request, cap, {f.reply_max}, &mut *scratch)?;")
                w("            let copy: &'a [u8; REQUEST_MAX] = scratch;")
                w("            let mut r = Reader::new(&copy[..length]);")
            else:
                w(f"            let mut copy = [0u8; {max(f.request_max, 1)}];")
                w(f"            let (call, length) = wire::take_buffer(request, cap, {f.reply_max}, &mut copy)?;")
                w("            let mut r = Reader::new(&copy[..length]);")
            for p in f.params:
                if p.type[0] == "bytes":
                    w(f"            let {p.name} = codec::decode_bytes::<{p.type[1]}>(&mut r).ok_or(Reject::Invalid)?;")
                else:
                    w(f"            let {p.name} = <{rust(p.type)} as Wire>::decode(&mut r).ok_or(Reject::Invalid)?;")
            w("            if !r.done() { return Err(Reject::Invalid); }")
            fields = ", ".join(p.name for p in f.params)
            w(f"            Ok((Request::{camel(f.name)}{' { ' + fields + ' }' if fields else ''}, call))")
        else:
            handle = f.handle
            kind = HANDLES[handle.type[2]] if handle else "CAP_KIND_NONE"
            w(f"            wire::body(request, cap, [{f.used[0]:#x}, {f.used[1]:#x}], {kind}, {'true' if handle else 'false'})?;")
            for p in f.params:
                if p.type[0] == "enum":
                    w(f"            let Some({p.name}) = {word_decode(p, 'words')} else {{ wire::discard(request, cap); return Err(Reject::Invalid) }};")
            fields = [f"{p.name}: cap" if p.type[0] == "handle" else (p.name if p.type[0] == "enum" else f"{p.name}: {word_decode(p, 'words')}") for p in f.params]
            w(f"            Ok((Request::{camel(f.name)}{' { ' + ', '.join(fields) + ' }' if fields else ''}, Call::words(request, cap)))")
        w("        }")
    w("        _ => { wire::discard(request, cap); Err(Reject::Invalid) }")
    w("    }")
    w("}")
    w("")
    for f in interface.functions:
        generate_reply(w, f)


def generate_reply(w, f):
    inner, optional, fallible, error = f.result if f.result else (None, False, False, None)
    name = f"reply_{f.name}"
    if inner is None:
        value_type = None
    elif inner[0] == "handle":
        value_type = "usize"
    elif f.buffered:
        value_type = rust_param(inner)
    else:
        value_type = rust(inner)
    if optional:
        value_type = f"Option<{value_type}>"
    if fallible:
        value_type = f"Result<{value_type or '()'}>"
    if error:
        value_type = f"core::result::Result<{value_type or '()'}, {camel(error)}>"
    signature = f"pub fn {name}(call: Call{', value: ' + value_type if value_type else ''}) -> Result<()> {{"
    if inner is not None and inner[0] == "handle":
        w(f"/// `value` is the handle of the capability to send ({'moved' if inner[1] == 'own' else 'copied'}).")
    w(signature)
    body_value = "value"
    if fallible:
        w("    let value = match value { Ok(value) => value, Err(error) => return wire::reply_error(call, error) };")
    if error:
        w("    let value = match value { Ok(value) => value, Err(error) => return wire::reply_code(call, error as usize) };")
    if optional:
        w("    let Some(value) = value else { return wire::reply_none(call) };")
    if f.buffered:
        if inner is None:
            w("    wire::reply_buffer(call, |_| Some(()))")
        else:
            w(f"    wire::reply_buffer(call, |w| {encode_expr(inner, body_value)})")
    elif inner is None:
        w("    wire::finish(call, [0, 0])")
    elif inner[0] == "handle":
        w(f"    wire::finish_cap(call, value, {'true' if inner[1] == 'own' else 'false'})")
    else:
        rf = f.result_field
        words = ["0", "0"]
        words[rf.word] = word_encode(rf, body_value)
        w(f"    wire::finish(call, [{words[0]}, {words[1]}])")
    w("}")


def main(argv):
    global ROOT, IDL_DIR, OUT_DIR
    if argv[:1] == ["--one"] and len(argv) == 3:
        source = Path(argv[1])
        try:
            Path(argv[2]).write_text(generate(parse(source.read_text(), source.as_posix()), source.as_posix()))
        except IdlError as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        return 0
    if "--root" in argv:  # generate for another tree (tests)
        ROOT = Path(argv[argv.index("--root") + 1]).resolve()
        IDL_DIR, OUT_DIR = ROOT / "idl", ROOT / "libmind" / "src" / "idl"
    check_only = "--check" in argv
    stale, names = [], []
    for path in sorted(IDL_DIR.glob("*.wit")):
        source = path.relative_to(ROOT).as_posix()
        try:
            interface = parse(path.read_text(), source)
        except IdlError as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        name = snake(interface.name)
        names.append(name)
        target = OUT_DIR / f"{name}.rs"
        text = generate(interface, source)
        if check_only:
            if not target.exists() or target.read_text() != text:
                stale.append(target.relative_to(ROOT).as_posix())
        else:
            target.write_text(text)
    module = ("// Generated by scripts/mind_idl.py; do not edit.\n//! Interfaces generated from idl/*.wit (MIND IDL v0.2).\n"
              "pub mod codec;\npub mod wire;\n" + "".join(f"pub mod {n};\n" for n in names))
    mod_path = OUT_DIR / "mod.rs"
    if check_only:
        if not mod_path.exists() or mod_path.read_text() != module:
            stale.append(mod_path.relative_to(ROOT).as_posix())
        if stale:
            print("stale generated files (run scripts/mind_idl.py): " + ", ".join(stale), file=sys.stderr)
            return 1
        print(f"MIND IDL: {len(names)} interface(s) up to date")
        return 0
    mod_path.write_text(module)
    print(f"MIND IDL: generated {len(names)} interface(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
