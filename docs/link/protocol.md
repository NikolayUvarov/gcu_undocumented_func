# The MIND Link protocol, version 1

Russian: [protocol_RU.md](protocol_RU.md).

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

## 5. Requests

Numbers are little-endian. A name is UTF-8, 1 to 255 bytes:
- one path component: no `/`, `\`, `:` and no control characters;
- not `.` or `..`.

The slave refuses any other name with DENIED.

### Control (channel 0)

**HELLO (1).**
- Request: `version u8 = 1`, `role u8 = 0` (master), `max_payload u16`, then the master's name (optional).
- Answer: `version u8`, `role u8 = 1` (slave), `max_payload u16`, then the slave's name (optional).
- `max_payload` is 64 to 1024; a slave takes a smaller one as 64. Each end then sends payloads no larger than the smaller `max_payload`.

**POLL (2).** Empty request, empty answer: the line is alive.

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
| LIST (16) | `cursor u32`, 0 for the first page | `next u32` (0xFFFFFFFF at the end), then lines `name<TAB>size<LF>` |
| STAT (17) | the name | `size u64`, then its SHA-256 (32 bytes); the master skips a put of the same bytes |
| DELETE (18) | the name | empty |

### Console (channel 1)

**CONSOLE (20).**
- Request: bytes typed at the master's end, possibly none.
- Answer: the slave's console output since the last answer, at most `max_payload` bytes.
- The master sends it as often as it wants output, an empty request included.

### Log (channel 3)

**LOG (30).**
- Request: `cursor u64`, 0 from the start of the log.
- Answer: `next u64`, then the log's bytes from `cursor`.
- The master asks again from `next`. An answer with no bytes means nothing new yet.

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

## 7. What the protocol does not do

The protocol makes no claims beyond these:
- **No security:** no authentication and no encryption. It is for a cable between the maintainer's machines.
  - A slave serves files only in its own directory: names are single components.
  - On MIND Core, the link service decides what a master may do (800-LNK-0003).
- **No integrity against an attacker:** the CRC and SHA-256 catch noise on the line, not an end that lies.
- **No guarantees from tests:** the host tests exercise a simulated line that loses, damages, repeats and echoes bytes. They are evidence for that line, not for a real one. A real line's evidence is recorded in the task that tests it.
