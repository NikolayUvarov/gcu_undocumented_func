#!/usr/bin/env python3
"""MIND IDL v0.2: generates libmind bindings from a WIT subset (docs/idl/README.md).

Usage: mind_idl.py                    write libmind/src/idl/<interface>.rs for every idl/*.wit
       mind_idl.py --check            fail if a generated file is missing or out of date
       mind_idl.py --one in.wit out.rs   generate one interface (tests)
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
WORD0_START, WORD_BITS = 16, 64  # word 0: method:8 | major:8 | fields
STRING_MAX, BYTES_MAX, LIST_MAX = 65535, 1 << 20, 4096


class IdlError(Exception):
    pass


@dataclass
class Type:
    """kind: int, enum (scalars, carried in the message words); string, bytes, list, record (bulk, in the buffer)."""
    kind: str
    name: str = ""       # integer type, enum or record name
    max: int = 0         # string/bytes bytes, list items
    item: "Type" = None  # list item

    @property
    def scalar(self):
        return self.kind in ("int", "enum")

    @property
    def bits(self):
        return INTEGERS[self.name] if self.kind == "int" else 8


@dataclass
class Field:
    name: str
    type: str  # source text of the type
    word: int = 0
    shift: int = 0
    t: Type = None

    @property
    def handle(self):
        match = re.fullmatch(r"(own|borrow)<(\w+)>", self.type)
        return (match.group(1), match.group(2)) if match else None

    @property
    def bits(self):
        return self.t.bits if self.t else INTEGERS[self.type]


@dataclass
class Function:
    name: str
    doc: list
    params: list
    result: str | None  # source text of the result type
    index: int = 0
    result_field: Field | None = None
    handle: Field | None = None
    used: list = field(default_factory=lambda: [0, 0])
    scalars: list = field(default_factory=list)
    bulk: list = field(default_factory=list)
    payload: Field | None = None   # implicit length of the bulk input
    ok: Type | None = None         # result value (None: no value)
    error: str | None = None       # enum of result<T, E>
    optional: bool = False
    ok_handle: tuple | None = None  # result<own|borrow<kind>, E>: a capability in the reply


@dataclass
class Record:
    name: str
    doc: list
    fields: list  # [(name, Type)]


@dataclass
class Enum:
    name: str
    doc: list
    cases: list


@dataclass
class Interface:
    package: str
    name: str
    version: tuple
    doc: list
    functions: list
    records: dict = field(default_factory=dict)
    enums: dict = field(default_factory=dict)


def snake(name):
    return name.replace("-", "_")


def camel(name):
    return "".join(part.capitalize() for part in re.split(r"[-_]", name))


def split_top(text):
    """Splits on commas that are not inside <...>."""
    parts, depth, current = [], 0, ""
    for ch in text:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(current.strip())
            current = ""
        else:
            current += ch
    if current.strip():
        parts.append(current.strip())
    return parts


def parse_type(text, interface, where):
    text = text.strip()
    if text in INTEGERS:
        return Type("int", text)
    if match := re.fullmatch(r"string<(\d+)>", text):
        size = int(match.group(1))
        if not 1 <= size <= STRING_MAX:
            raise IdlError(f"{where}: string size 1..{STRING_MAX}")
        return Type("string", max=size)
    if match := re.fullmatch(r"bytes<(\d+)>", text):
        size = int(match.group(1))
        if not 1 <= size <= BYTES_MAX:
            raise IdlError(f"{where}: bytes size 1..{BYTES_MAX}")
        return Type("bytes", max=size)
    if match := re.fullmatch(r"list<(.+),\s*(\d+)>", text):
        item = parse_type(match.group(1), interface, where)
        count = int(match.group(2))
        if item.kind not in ("int", "enum", "record") or not 1 <= count <= LIST_MAX:
            raise IdlError(f"{where}: list items are integers, enums or records; 1..{LIST_MAX} of them")
        return Type("list", max=count, item=item)
    if re.fullmatch(r"[\w-]+", text):
        name = snake(text)
        if name in interface.enums:
            return Type("enum", name)
        if name in interface.records:
            return Type("record", name)
    raise IdlError(f"{where}: unsupported type '{text}'")


def layout(fields, start=WORD0_START):
    """Places scalar fields in declaration order; a field never straddles words. Returns the used-bit masks."""
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


def parse(text, source="<idl>"):
    lines = text.splitlines()
    package = name = None
    version, doc, pending, functions, depth = None, [], [], [], 0
    interface = Interface("", "", (0, 0, 0), [], [])
    raw_records, block = [], None  # block: ("record"|"enum", name, doc, [lines])
    for number, raw in enumerate(lines, 1):
        line = raw.strip()
        where = f"{source}:{number}"
        if not line:
            continue
        if line.startswith("///"):
            pending.append(line[3:].strip())
            continue
        if line.startswith("//"):
            continue
        if block:
            if line == "}":
                kind, bname, bdoc, body = block
                items = [p for p in split_top(" ".join(body)) if p]
                if bname in ("request", "reject", "status", "buffer", "result", "ipc", "wire"):
                    raise IdlError(f"{where}: '{bname}' is reserved in the generated bindings")
                if kind == "enum":
                    if not 1 <= len(items) <= 255:
                        raise IdlError(f"{where}: an enum has 1..255 cases")
                    interface.enums[bname] = Enum(bname, bdoc, [snake(c) for c in items])
                else:
                    raw_records.append((bname, bdoc, items, where))
                block = None
            else:
                body = block[3]
                body.append(re.sub(r"///.*", "", line))
            continue
        if match := re.fullmatch(r"package\s+([\w-]+:[\w-]+)@(\d+)\.(\d+)\.(\d+);", line):
            package, version = match.group(1), tuple(int(match.group(i)) for i in (2, 3, 4))
            doc, pending = doc + pending, []
            continue
        if match := re.fullmatch(r"interface\s+([\w-]+)\s*\{", line):
            if name or depth:
                raise IdlError(f"{where}: one interface per file")
            name, depth = match.group(1), 1
            doc, pending = doc + pending, []
            continue
        if match := re.fullmatch(r"(record|enum)\s+([\w-]+)\s*\{(.*?)(\})?", line):
            if depth != 1:
                raise IdlError(f"{where}: {match.group(1)} outside the interface")
            block = (match.group(1), snake(match.group(2)), pending, [match.group(3)])
            pending = []
            if match.group(4):
                lines.insert(number, "}")  # one-line declaration: close it on the next line
            continue
        if line == "}":
            depth -= 1
            continue
        if match := re.fullmatch(r"([\w-]+)\s*:\s*func\((.*)\)\s*(?:->\s*(.+?))?\s*;", line):
            if depth != 1:
                raise IdlError(f"{where}: function outside the interface")
            params = []
            for part in split_top(match.group(2)):
                pm = re.fullmatch(r"([\w-]+)\s*:\s*(.+)", part)
                if not pm:
                    raise IdlError(f"{where}: bad parameter '{part}'")
                params.append(Field(snake(pm.group(1)), pm.group(2).strip()))
            functions.append(Function(snake(match.group(1)), pending, params, match.group(3)))
            pending = []
            continue
        raise IdlError(f"{where}: unsupported syntax: {line}")
    if not package or not name or depth != 0 or block:
        raise IdlError(f"{source}: needs a package line and one closed interface")
    if version[0] == 0 or version[0] > 255:
        raise IdlError(f"{source}: major version must be 1..255")
    if not 1 <= len(functions) <= 255:
        raise IdlError(f"{source}: 1..255 functions")
    interface.package, interface.name, interface.version, interface.doc, interface.functions = package, name, version, doc, functions
    # Records may refer to records declared before them (no recursion).
    for rname, rdoc, items, where in raw_records:
        interface.records[rname] = None
    for rname, rdoc, items, where in raw_records:
        fields = []
        for item in items:
            fm = re.fullmatch(r"([\w-]+)\s*:\s*(.+)", item)
            if not fm:
                raise IdlError(f"{where}: bad record field '{item}'")
            t = parse_type(fm.group(2), interface, where)
            if t.kind == "list":
                raise IdlError(f"{where}: lists are not allowed inside records")
            if t.kind == "record" and interface.records.get(t.name) is None:
                raise IdlError(f"{where}: record '{t.name}' must be declared before '{rname}'")
            fields.append((snake(fm.group(1)), t))
        if not fields:
            raise IdlError(f"{where}: empty record")
        interface.records[rname] = Record(rname, rdoc, fields)
    for index, function in enumerate(functions, 1):
        check(function, interface, f"{source}: {function.name}")
        function.index = index
    return interface


def check(function, interface, where):
    for p in function.params:
        if p.handle:
            if p.handle[1] not in HANDLES:
                raise IdlError(f"{where}: unknown handle kind '{p.handle[1]}'")
            if function.handle:
                raise IdlError(f"{where}: at most one capability per message")
            function.handle = p
            continue
        p.t = parse_type(p.type, interface, where)
        (function.scalars if p.t.scalar else function.bulk).append(p)
    result = function.result
    if result is not None:
        if match := re.fullmatch(r"result<(.+),\s*([\w-]+)>", result):
            ok, error = match.group(1).strip(), snake(match.group(2))
            if error not in interface.enums:
                raise IdlError(f"{where}: the error of result<> must be an enum")
            function.error = error
            if handle := re.fullmatch(r"(own|borrow)<(\w+)>", ok):
                if handle.group(2) not in HANDLES:
                    raise IdlError(f"{where}: unknown handle kind '{handle.group(2)}'")
                function.ok_handle = (handle.group(1), handle.group(2))
                function.ok = None
            else:
                function.ok = None if ok == "_" else parse_type(ok, interface, where)
        elif match := re.fullmatch(r"option<(\w+)>", result):
            function.optional = True
            function.ok = parse_type(match.group(1), interface, where)
            if function.ok.kind != "int":
                raise IdlError(f"{where}: option<> holds an integer")
        else:
            function.ok = parse_type(result, interface, where)
        if function.ok and function.ok.scalar:
            function.result_field = Field("value", function.ok.name if function.ok.kind == "int" else "u8", t=function.ok)
            layout([function.result_field])
    needs_buffer = function.bulk or (function.ok and not function.ok.scalar)
    if needs_buffer and not (function.handle and function.handle.handle == ("borrow", "memory")):
        raise IdlError(f"{where}: strings, bytes, lists and records travel in a borrow<memory> buffer parameter")
    scalars = list(function.scalars)
    if function.bulk:
        function.payload = Field("payload", "u32")
        scalars = [function.payload] + scalars
    function.used = layout(scalars)


# --- Rust generation --------------------------------------------------------------------------------------------

def rust_int(t):
    return {"bool": "bool", "u8": "u8", "u16": "u16", "u32": "u32", "u64": "u64"}[t]


def needs_life(t, interface):
    if t.kind in ("string", "bytes", "list"):
        return True
    if t.kind == "record":
        return any(needs_life(ft, interface) for _, ft in interface.records[t.name].fields)
    return False


def rust_type(t, interface, life="'a", view=False):
    """Rust type for a value: `view` gives the decoded form of a list (a List view), otherwise a slice."""
    if t.kind == "int":
        return rust_int(t.name)
    if t.kind == "enum":
        return camel(t.name)
    if t.kind == "string":
        return f"&{life} str"
    if t.kind == "bytes":
        return f"&{life} [u8]"
    if t.kind == "record":
        return camel(t.name) + (f"<{life}>" if needs_life(t, interface) else "")
    if t.kind == "list":
        item = rust_type(t.item, interface, life)
        return f"wire::List<{life}, {item}>" if view else f"&{life} [{item}]"
    raise AssertionError(t)


def write_value(t, expr, interface):
    if t.kind == "int":
        return f"w.{t.name}({expr})?;"
    if t.kind == "enum":
        return f"w.u8({expr} as u8)?;"
    if t.kind == "string":
        return f"w.str({expr}, {t.max})?;"
    if t.kind == "bytes":
        return f"w.bytes({expr}, {t.max})?;"
    if t.kind == "record":
        return f"wire::Item::encode(&{expr}, w)?;" if not expr.startswith("&") else f"wire::Item::encode({expr}, w)?;"
    if t.kind == "list":
        return f"w.list({expr}, {t.max})?;"
    raise AssertionError(t)


def read_value(t, interface, life="'a"):
    if t.kind == "int":
        return f"r.{t.name}()?"
    if t.kind == "enum":
        return f"{camel(t.name)}::from_u8(r.u8()?)?"
    if t.kind == "string":
        return f"r.str({t.max})?"
    if t.kind == "bytes":
        return f"r.bytes({t.max})?"
    if t.kind == "record":
        return f"<{rust_type(t, interface, life)} as wire::Item>::decode(r)?"
    if t.kind == "list":
        return f"wire::List::read(r, {t.max})?"
    raise AssertionError(t)


def encode_word(f, expr):
    value = f"({expr} as u8)" if f.t and f.t.kind == "enum" else expr
    return f"(({value}) as usize) << {f.shift}"


def decode_word(f, words):
    value = f"wire::field(&{words}, {f.word}, {f.shift}, {f.bits})"
    if f.t and f.t.kind == "enum":
        return f"{camel(f.t.name)}::from_u8({value} as u8)"
    if f.type == "bool" or (f.t and f.t.kind == "int" and f.t.name == "bool"):
        return f"{value} != 0"
    return f"{value} as {rust_int(f.t.name if f.t else f.type)}"


def generate(interface, source):
    out = []
    w = out.append
    major, minor, patch = interface.version
    w(f"// Generated by scripts/mind_idl.py from {source}; do not edit.")
    for line in interface.doc:
        w(f"//! {line}" if line else "//!")
    w("#![allow(clippy::all, unused_imports, unused_variables, unused_mut, dead_code)]")
    w("use crate::abi::*;")
    w("use crate::ipc::{self, Received};")
    w("use crate::sys::{Error as SysError, Result};")
    w("use super::wire::{self, Reject};")
    w("")
    w(f'pub const PACKAGE: &str = "{interface.package}";')
    w(f"pub const VERSION: (u8, u8, u8) = ({major}, {minor}, {patch});")
    w(f"const MAJOR: usize = {major};")
    w("")
    # Enums.
    for e in interface.enums.values():
        for line in e.doc:
            w(f"/// {line}")
        name = camel(e.name)
        w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
        w("#[repr(u8)]")
        w(f"pub enum {name} {{ {', '.join(f'{camel(c)} = {i}' for i, c in enumerate(e.cases))} }}")
        w(f"impl {name} {{")
        w(f"    pub fn from_u8(value: u8) -> Option<Self> {{ match value {{ {' '.join(f'{i} => Some(Self::{camel(c)}),' for i, c in enumerate(e.cases))} _ => None }} }}")
        w("}")
        w(f"impl<'a> wire::Item<'a> for {name} {{")
        w("    fn encode(&self, w: &mut wire::Writer) -> Result<()> { w.u8(*self as u8) }")
        w("    fn decode(r: &mut wire::Reader<'a>) -> Option<Self> { Self::from_u8(r.u8()?) }")
        w("}")
        w("")
    # Records.
    for rec in interface.records.values():
        for line in rec.doc:
            w(f"/// {line}")
        rt = Type("record", rec.name)
        life = needs_life(rt, interface)
        name = camel(rec.name)
        w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
        w(f"pub struct {name}{'<' + chr(39) + 'a>' if life else ''} {{ {', '.join(f'pub {n}: {rust_type(t, interface)}' for n, t in rec.fields)} }}")
        w(f"impl<'a> wire::Item<'a> for {name}{'<' + chr(39) + 'a>' if life else ''} {{")
        w("    fn encode(&self, w: &mut wire::Writer) -> Result<()> {")
        for n, t in rec.fields:
            w(f"        {write_value(t, ('&self.' if t.kind == 'record' else 'self.') + n, interface)}")
        w("        Ok(())")
        w("    }")
        w(f"    fn decode(r: &mut wire::Reader<'a>) -> Option<Self> {{ Some(Self {{ {', '.join(f'{n}: {read_value(t, interface)}' for n, t in rec.fields)} }}) }}")
        w("}")
        w("")
    # Client side.
    for f in interface.functions:
        for line in f.doc:
            w(f"/// {line}")
        buffered = f.handle is not None and (f.bulk or (f.ok and not f.ok.scalar))
        life = "<'b>" if buffered else ""
        args = ["endpoint: ipc::Endpoint"]
        for p in f.params:
            if p.handle:
                args.append(f"{p.name}: wire::Buffer<'b>" if buffered else f"{p.name}: usize")
            elif p.t.scalar:
                args.append(f"{p.name}: {rust_type(p.t, interface)}")
        for p in f.bulk:
            args.append(f"{p.name}: {rust_type(p.t, interface, chr(39) + '_')}")
        if f.ok_handle:
            args.append("receive: usize")
        value = "usize" if f.ok_handle else "()" if f.ok is None else rust_type(f.ok, interface, "'b", view=True)
        if f.optional:
            value = f"Option<{value}>"
        ret = f"core::result::Result<{value}, {camel(f.error)}>" if f.error else value
        w(f"pub fn {f.name}{life}({', '.join(args)}) -> Result<{ret}> {{")
        if buffered:
            w(f"    let wire::Buffer {{ cap, bytes }} = {f.handle.name};")
            w("    let mut writer = wire::Writer::new(bytes);")
            if f.bulk:
                w("    {")
                w("        let w = &mut writer;")
                for p in f.bulk:
                    w(f"        {write_value(p.t, p.name, interface)}")
                w("    }")
            w("    let payload = writer.len();")
            w("    let bytes = writer.into_inner();")
        words = [[f"{f.index}", "MAJOR << 8"], []]
        if f.payload:
            words[f.payload.word].append(encode_word(f.payload, "payload"))
        for p in f.scalars:
            words[p.word].append(encode_word(p, p.name))
        w(f"    let words = [{' | '.join(words[0])}, {' | '.join(words[1]) or '0'}];")
        sent = "None"
        if f.handle:
            mode, _ = f.handle.handle
            cap = "cap" if buffered else f.handle.name
            sent = f"Some(({cap}, {'true' if mode == 'own' else 'false'}))"
        if f.ok_handle:
            w(f"    let (reply, received) = wire::call_receiving(endpoint, words, {sent}, receive)?;")
        else:
            w(f"    let reply = wire::call(endpoint, words, {sent})?;")
        # Reply: status, then the value (scalar in the words, bulk in the buffer).
        if f.ok is not None and f.ok.scalar:
            rf = f.result_field
            used = [0, 0]
            used[rf.word] = ((1 << rf.bits) - 1) << rf.shift
        elif f.ok is not None:
            used = [0xFFFF_FFFF << 16, 0]
        else:
            used = [0, 0]
        w(f"    let status = wire::check_reply(&reply, [{used[0]:#x}, {used[1]:#x}], {'true' if f.optional else 'false'}, {'true' if f.error else 'false'})?;")
        if f.error:
            w(f"    if let wire::Status::Failed(code) = status {{ return {camel(f.error)}::from_u8(code).map(Err).ok_or(SysError::Invalid); }}")
        if f.ok_handle:
            w(f"    if !received || crate::dev::cap_info(receive).0 != {HANDLES[f.ok_handle[1]]} {{ return Err(SysError::Invalid); }}")
            value_expr = "receive"
        elif f.ok is None:
            value_expr = "()"
        elif f.ok.scalar:
            value_expr = decode_word(f.result_field, "reply")
            if f.ok.kind == "enum":
                value_expr += ".ok_or(SysError::Invalid)?"
        else:
            w("    let len = wire::field(&reply, 0, 16, 32);")
            w("    if len > bytes.len() { return Err(SysError::Invalid); }")
            w("    let bytes: &'b [u8] = bytes;")
            w("    let mut reader = wire::Reader::new(&bytes[..len]);")
            w("    let value = (|r: &mut wire::Reader<'b>| -> Option<_> { let value = " + read_value(f.ok, interface, "'b") + "; r.end().then_some(value) })(&mut reader).ok_or(SysError::Invalid)?;")
            value_expr = "value"
        if f.optional:
            value_expr = f"if status == wire::Status::None {{ None }} else {{ Some({value_expr}) }}"
        w(f"    Ok({'Ok(' + value_expr + ')' if f.error else value_expr})")
        w("}")
        w("")
    # Server side.
    w(f"/// A request to the `{interface.name}` interface that passed the receiver's schema check.")
    w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    w("pub enum Request {")
    for f in interface.functions:
        fields = []
        if f.payload:
            fields.append("payload: u32")
        for p in f.params:
            if p.handle:
                fields.append(f"{p.name}: usize")
            elif p.t.scalar:
                fields.append(f"{p.name}: {rust_type(p.t, interface)}")
        w(f"    {camel(f.name)}" + (f" {{ {', '.join(fields)} }}," if fields else ","))
    w("}")
    w("")
    w("/// Checks a received message against the schema (MC-2.4): method, major version, unused bits, enum values, capability")
    w("/// kind. `cap` is the slot passed to `recv`; an unexpected capability is dropped (MC-2.12). Bulk arguments are checked")
    w("/// by `args_<function>` once the server has mapped the buffer.")
    w("pub fn decode(request: &Received, cap: usize) -> core::result::Result<Request, Reject> {")
    w("    let words = request.data;")
    w("    wire::header(request, cap, MAJOR)?;")
    w("    match words[0] & 0xFF {")
    for f in interface.functions:
        handle = f.handle
        kind = HANDLES[handle.handle[1]] if handle else None
        w(f"        {f.index} => {{")
        w(f"            wire::body(request, cap, [{f.used[0]:#x}, {f.used[1]:#x}], {kind if kind else 'CAP_KIND_NONE'}, {'true' if handle else 'false'})?;")
        fields = []
        if f.payload:
            fields.append(f"payload: {decode_word(f.payload, 'words')}")
        for p in f.params:
            if p.handle:
                fields.append(f"{p.name}: cap")
            elif p.t.scalar:
                value = decode_word(p, "words")
                if p.t.kind == "enum":
                    value = f"match {value} {{ Some(v) => v, None => {{ wire::discard(request, cap); return Err(Reject::Invalid); }} }}"
                fields.append(f"{p.name}: {value}")
        if fields:
            w(f"            Ok(Request::{camel(f.name)} {{ {', '.join(fields)} }})")
        else:
            w(f"            Ok(Request::{camel(f.name)})")
        w("        }")
    w("        _ => { wire::discard(request, cap); Err(Reject::Invalid) }")
    w("    }")
    w("}")
    w("")
    for f in interface.functions:
        if f.bulk:
            types = [rust_type(p.t, interface, "'a", view=True) for p in f.bulk]
            tuple_type = types[0] if len(types) == 1 else f"({', '.join(types)})"
            w(f"/// Bulk arguments of `{f.name}` from the mapped buffer: exactly `payload` bytes, every limit checked.")
            w(f"pub fn args_{f.name}<'a>(bytes: &'a [u8], payload: u32) -> core::result::Result<{tuple_type}, Reject> {{")
            w("    let len = payload as usize;")
            w("    if len > bytes.len() { return Err(Reject::Invalid); }")
            w("    let mut reader = wire::Reader::new(&bytes[..len]);")
            reads = ", ".join(f"{read_value(p.t, interface)}" for p in f.bulk)
            tuple_expr = reads if len(f.bulk) == 1 else f"({reads})"
            w(f"    let value = (|r: &mut wire::Reader<'a>| -> Option<_> {{ let value = {tuple_expr}; r.end().then_some(value) }})(&mut reader);")
            w("    value.ok_or(Reject::Invalid)")
            w("}")
            w("")
    for f in interface.functions:
        bulk_result = f.ok is not None and not f.ok.scalar
        if f.ok_handle:
            value_type = "usize"
        elif f.ok is None:
            value_type = "()"
        else:
            value_type = rust_type(f.ok, interface, "'_")
        if f.error:
            value_type = f"core::result::Result<{value_type}, {camel(f.error)}>"
        elif f.optional:
            value_type = f"Option<{value_type}>"
        args = (["bytes: &mut [u8]"] if bulk_result else []) + ([] if (f.ok is None and not f.error and not f.ok_handle) else [f"value: {value_type}"])
        w(f"pub fn reply_{f.name}({', '.join(args)}) -> Result<()> {{")
        ok_value = "value"
        if f.error:
            w(f"    let value = match value {{ Ok(value) => value, Err(code) => return wire::reply([wire::STATUS_FAILED | (code as usize) << 16, 0]) }};")
        if f.optional:
            w("    let Some(value) = value else { return wire::reply([wire::STATUS_NONE, 0]) };")
        if f.ok_handle:
            w(f"    wire::reply_cap([0, 0], value, {'true' if f.ok_handle[0] == 'own' else 'false'})")
        elif f.ok is None:
            w("    wire::reply([0, 0])")
        elif f.ok.scalar:
            rf = f.result_field
            words = ["0", "0"]
            words[rf.word] = encode_word(rf, "value")
            w(f"    wire::reply([{words[0]}, {words[1]}])")
        else:
            w("    let mut writer = wire::Writer::new(bytes);")
            w(f"    let encoded = (|w: &mut wire::Writer| -> Result<()> {{ {write_value(f.ok, 'value', interface)} Ok(()) }})(&mut writer);")
            w("    if encoded.is_err() { return wire::reply([wire::STATUS_OVERFLOW, 0]); }")
            w("    wire::reply([(writer.len() as usize) << 16, 0])")
        w("}")
    w("")
    return "\n".join(out)


def main(argv):
    if argv[:1] == ["--one"] and len(argv) == 3:
        # One interface to a given file (the generator's own tests).
        source = Path(argv[1])
        Path(argv[2]).write_text(generate(parse(source.read_text(), source.as_posix()), source.as_posix()))
        return 0
    check_only = "--check" in argv
    stale = []
    names = []
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
    module = "// Generated by scripts/mind_idl.py; do not edit.\n//! Interfaces generated from idl/*.wit (MIND IDL v0.2).\npub mod wire;\n" + "".join(f"pub mod {n};\n" for n in names)
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
