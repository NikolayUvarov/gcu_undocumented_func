# The MIND Link protocol, version 1

Russian: [protocol_RU.md](protocol_RU.md).

**Additions of 2026-10-20** within version 1:
- paths below the slave's directory, and requests for directories (800-LNK-0010);
- the programs channel (800-LNK-0009);
- two refusal codes.

A slave from before them refuses the new requests with UNSUPPORTED, and a name with `/` with DENIED, so a master can tell. The frame and its version are unchanged.

MIND Link connects a development machine to a machine under test over any byte stream (task [800](../../issues/800-mind-link.md)):
- a serial line through USB–RS-485 converters;
- a CDC-ACM port;
- a vendor bulk pipe of the RP2 multi-device.

This page describes the protocol well enough to write a third implementation. The reference implementation is the `no_std` crate [linkproto](../../linkproto/src/lib.rs). Its host tests are in [linkproto/tests/link.rs](../../linkproto/tests/link.rs).

What this page does not cover yet:
- the transports' own settings;
- the RP2 boards' own protocol between them ([800-LNK-0005](../../issues/800-LNK-0005-the-rp2-multi-device-firmware.md)).

## 1. Roles

One end is the **master**, the other the **slave**.

- **Only the master starts an exchange.** It sends a request and waits for the answer.
- **The slave answers each request once** and never sends unasked. A half-duplex line, such as two-wire RS-485, therefore never has two senders.
- **What the slave has to say waits for a request.** Console output and log lines go back in the answer to the master's next console or log request.

Either machine may be the master. The development machine usually is.

## 2. Frames on the line

A frame is:

| Bytes | Field | Meaning |
|---|---|---|
| 1 | version | 1 |
| 1 | channel | what the frame belongs to (section 4) |
| 1 | kind | the request, or the answer (section 5) |
| 1 | flags | 0; ignored by version 1 |
| 2 | seq | sequence number, little-endian: pairs a request with its answer |
| 2 | len | the payload's length, little-endian, at most 1024 |
| len | payload | |
| 4 | crc | CRC-32C of everything before it, little-endian |

**CRC-32C** is the Castagnoli CRC: reflected polynomial 0x82F63B78, initial value 0xFFFFFFFF, final XOR 0xFFFFFFFF. The CRC of the ASCII string `123456789` is 0xE3069283.

**On the line**, the whole frame is COBS-encoded (Consistent Overhead Byte Stuffing), so it holds no zero byte, and a single zero byte follows it.

A receiver:
- collects bytes up to a zero byte, then decodes them;
- drops the frame, counting it, when any of these hold:
  - the bytes do not decode;
  - the frame is shorter than 12 bytes;
  - the version is not 1;
  - `len` is over 1024 or does not match the frame's length;
  - the CRC does not match;
- drops everything up to the next zero byte when more than 1 042 bytes come without one (a frame of at most 1 036 bytes takes at most 1 041 once COBS-encoded);
- treats a zero byte with nothing before it as nothing, so a sender may send a zero byte first to end any noise on the line.

## 3. Exchanges

**The master:**
1. increases `seq` by 1, wrapping at 65 535 (its first `seq` comes from its clock, so a restarted master's requests do not repeat its old ones);
2. sends the request;
3. waits for a frame with the same `seq` and channel, and its kind with bit 0x80 set;
4. ignores any other frame:
   - a frame without bit 0x80 is its own echo (some RS-485 converters hear what they send);
   - another `seq` is an answer to an earlier try;
5. when no answer comes within its wait, sends the same request again, with the same `seq`, a bounded number of times (the reference: 500 ms, 8 tries; the wait must exceed a full frame's time on the line at its speed);
6. then fails the exchange with a timeout.

**The slave:**
1. ignores frames with bit 0x80 set: they are its own echo;
2. when a request has the same channel, kind, `seq` and payload CRC as the last one it answered, sends its last answer again and does not carry the request out again. An answer lost on the line therefore never makes a write happen twice;
3. otherwise carries the request out and answers with:
   - **an answer:** the request's kind with bit 0x80 set, its channel and `seq`, and the answer's payload;
   - **or a refusal:** kind 0xFF, its channel and `seq`, and a one-byte payload, the code (section 6);
4. may wait a moment before it answers, so that a half-duplex converter turns its line round.

Each request is answered before the next is sent: stop and wait. A transfer's speed is therefore a payload of 1 024 bytes per round trip.

## 4. Channels

| Channel | Name | Carries |
|---|---|---|
| 0 | control | hello, poll |
| 1 | console | console bytes both ways |
| 2 | files | put, get, list, stat, delete |
| 3 | log | the slave's system log |
| 4 | input | keyboard and mouse events (reserved for the multi-device) |
| 5 | programs | programs the slave starts for the master, and their output |

## 5. Requests

Numbers are little-endian. A name is a path below the slave's directory: UTF-8, 1 to 255 bytes in all.
- **Its parts** are joined by single `/`. Each part:
  - holds no `\`, `:` or control character;
  - is neither empty, nor `.`, nor `..`.
- **So it never leaves the directory:** no leading `/`, no `..`, no drive or volume.

The slave refuses any other name with DENIED. Before the additions of 2026-10-20, a name was one part.

### Control (channel 0)

**HELLO (1).**
- Request: `version u8 = 1`, `role u8 = 0` (master), `max_payload u16`, then the master's name (optional).
- Answer: `version u8`, `role u8 = 1` (slave), `max_payload u16`, then the slave's name (optional).
- `max_payload` is 64 to 1024; a slave takes a smaller one as 64. Each end then sends payloads no larger than the smaller `max_payload`. A master may announce less than it can take, for a line that loses bytes or a converter with a small buffer.

**POLL (2).** Empty request, empty answer: the line is alive.

**REBOOT (3)** (800-LNK-0012). Empty request, empty answer. The slave's machine restarts once the answer is out.
- MIND Core's `link` does so only with the word `dev` in `link.conf` (DENIED otherwise) and the lifecycle client `init` grants with it (UNSUPPORTED without it).
- `mindlink serve` refuses it with UNSUPPORTED.

### Files (channel 2)

**Put.** A file is sent in order from offset 0 and checked by SHA-256 at the end.

**PUT_OPEN (10).**
- Request: `size u64`, then the name.
- Answer: `resume u64`.
- `resume` is how much the slave holds already:
  - an earlier put of the same name and size was interrupted: the bytes it received;
  - otherwise 0, and the slave starts the file afresh, dropping any unfinished put.
- The master continues from `resume`, hashing the first `resume` bytes from its own copy.

**PUT_DATA (11).**
- Request: `offset u64`, then the data.
- `offset` must equal the bytes received so far, and the data must not pass `size`; otherwise BAD_REQUEST.
- Empty answer.

**PUT_CLOSE (12).**
- Request: the file's SHA-256 (32 bytes).
- Empty answer when the slave received `size` bytes whose SHA-256 is this one: the file is kept.
- Otherwise the slave refuses with MISMATCH and does not keep the file.

**Get.** A file is read in order from offset 0 and checked by SHA-256 at the end.

**GET_OPEN (13).**
- Request: the name.
- Answer: `size u64`. NOT_FOUND when there is no such file.

**GET_DATA (14).**
- Request: `offset u64`, `max u16`.
- `offset` must equal the bytes sent so far and be below `size`.
- Answer: 1 to `max` bytes of the file from `offset`.

**GET_CLOSE (15).**
- Empty request.
- Answer: the SHA-256 of the bytes the slave sent. The master compares it with its own.

**The others:**

| Request | Payload | Answer |
|---|---|---|
| LIST (16) | `cursor u32`, 0 for the first page | `next u32` (0xFFFFFFFF at the end), then lines `name<TAB>size<LF>`: the files at the directory's top |
| STAT (17) | the name | `size u64`, then its SHA-256 (32 bytes); the master skips a put of the same bytes |
| DELETE (18) | the name of a file | empty |

**A put into a directory that is missing** is refused at PUT_OPEN with NOT_FOUND. A put whose name is a directory is refused with EXISTS.

**Directories** (800-LNK-0010):

| Request | Payload | Answer |
|---|---|---|
| MKDIR (19) | the name | empty. Missing parents are made too; a directory that exists is no error; a file of that name is EXISTS |
| MOVE (21) | `flags u8` (bit 0: replace), `from_len u8`, the name moved, then its new name | empty. A file or a whole directory. A file at the new name is replaced only with bit 0, a directory never (EXISTS). A missing parent of the new name is NOT_FOUND; a directory moved into itself, DENIED |
| RMDIR (22) | `flags u8` (bit 0: recursive), the name | empty. Without bit 0, only an empty directory (NOT_EMPTY otherwise); with it, the directory and everything in it. A file is DENIED |
| LIST_DIR (23) | `cursor u32`, then the directory's name, none for the top | as LIST, for that directory: its files, and its directories, whose names end in `/` and whose size is 0 |

### Console (channel 1)

**CONSOLE (20).**
- Request: bytes typed at the master's end, possibly none.
- Answer: the slave's console output since the last answer, at most `max_payload` bytes.
- The master sends it as often as it wants output, an empty request included.

### Log (channel 3)

**LOG (30).**
- Request: `cursor u64`: 0 from the start of the log, else the `next` of the last answer.
- Answer: `next u64`, then the log's text from `cursor`, whole lines.
- What the cursor counts is the slave's: `mindlink serve` counts bytes of its log file, MIND Core's `link` the system log's sequence numbers. The master only hands it back.
- The master asks again from `next`. An answer with no text means nothing new yet.

### Programs (channel 5)

The slave starts a program from its directory for the master, keeps its output and tells its end (800-LNK-0009).
- **On MIND Core** only when `link.conf` holds the word `run`, through the loader as an application starts one (800-KRN-0132).
  - **With `dev`** (the developer mode, 800-LNK-0012), a name of one part without `.` (`wifi`, `netcheck`) is a built-in program from the volume's root.
  - **Also with `dev`,** the programs get the network, the Wi-Fi station and the system monitor where they ask for them and `init` granted `link` those.
- **`mindlink serve`** only with `--run`.
- **Without it,** RUN is refused with DENIED.

| Request | Payload | Answer |
|---|---|---|
| RUN (50) | `name_len u8`, the program's name, then its arguments (UTF-8, the rest) | `run u32`, `pid u64`: the run's number, and the program's process |
| OUTPUT (51) | `run u32`, `cursor u64` | `state u8`, `exit u32`, `from u64`, then output from offset `from` |
| STOP (52) | `run u32` | empty: the program is ended |
| RUNS (53) | `cursor u32` | `next u32`, then lines `run<TAB>pid<TAB>state<TAB>name<LF>`, `state` one of `running`, `ended`, `stopped`, `faulted` |
| COMMAND (54) | one line for the slave's shell, UTF-8, at most 240 bytes, no control characters | `run u32`: a run whose output and end OUTPUT reads (800-LNK-0015) |
| REPLACE (55) | `unit_len u8`, the service's name (at most 16 bytes), then a file's name under the store (at most 64 bytes; empty: its boot image) | empty: the service was started again from it, unverified (800-LNK-0016; MIND Core: `dev` in `link.conf`, `init.wit` 1.6 `replace`) |

- **Output** is counted from the program's start.
  - The slave keeps the last part of it: at least 16 KiB on MIND Core, 64 KiB in `mindlink serve`.
  - `from` is the cursor, or more when the bytes from the cursor are no longer kept.
  - The master asks again from `from` plus the bytes it got.
- **`state`:** 0 running, 1 ended, 2 stopped, 3 ended by a fault.
  - Once the state is not 0, an answer with no bytes means there is no more output.
- **`exit`:** the program's exit code; for state 3, the fault's vector; 0xFFFFFFFF while unknown.
  - MIND Core's `link` learns it from the loader, which watches the program for it (800-KRN-0138). With an older loader it stays unknown.
- **A run's number** is the slave's, and a restarted slave starts new numbers. A run it does not know is NOT_FOUND.
- **COMMAND** runs a line in the slave's shell.
  - The shell's answer is the run's output, and its end is the run's end: exit 0, or 1 with the shell's reason when it refused or failed the line.
  - `mindlink serve --run` runs the line with `sh -c`.
  - MIND Core's `link` will pass it to the shell's `run` (`idl/shell.wit`) with `dev` in `link.conf`, once `init` gives it a client of the shell. Until then it answers UNSUPPORTED.
- **STOP** is refused with UNSUPPORTED where the slave cannot end a program. MIND Core's `link` asks the loader, which ends only the programs it started for `link` (800-KRN-0138).

### Input (channel 4)

**INPUT (40).** Reserved for keyboard and mouse events. Its payload is defined with the multi-device ([800-LNK-0005](../../issues/800-LNK-0005-the-rp2-multi-device-firmware.md)). Version 1's slaves refuse it with UNSUPPORTED.

## 6. Refusal codes

| Code | Name | When |
|---|---|---|
| 1 | NOT_FOUND | no such file |
| 2 | DENIED | the name is not allowed, or the slave does not allow the request |
| 3 | IO | the slave's storage failed |
| 4 | MISMATCH | a put's SHA-256 or size does not match |
| 5 | BAD_REQUEST | a payload too short, an offset out of order, no put or get open |
| 6 | BUSY | the slave cannot do it now |
| 7 | UNSUPPORTED | an unknown channel or kind, or a request this slave does not serve |
| 8 | NOT_EMPTY | a directory to remove without `recursive` holds something |
| 9 | EXISTS | something of that name is in the way |

## 7. What the protocol does not do

The protocol makes no claims beyond these:
- **No security:** no authentication and no encryption. It is for a cable between the maintainer's machines.
  - A slave serves files only in its own directory: no name leaves it.
  - On MIND Core, the link service decides what a master may do (800-LNK-0003). `vfs_server` keeps its files inside `data/link/` whatever it checks.
  - Starting programs is a power the slave's owner gives: `run` in `link.conf`, or `--run` for `mindlink serve`. Whoever holds the cable then runs programs as that end allows: on MIND Core with an application's grants, under `mindlink serve` as the user who serves.
- **No integrity against an attacker:** the CRC and SHA-256 catch noise on the line, not an end that lies.
- **No guarantees from tests:** the host tests exercise a simulated line that loses, damages, repeats and echoes bytes. They are evidence for that line, not for a real one. A real line's evidence is recorded in the task that tests it.
