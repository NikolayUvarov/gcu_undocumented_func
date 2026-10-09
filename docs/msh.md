# msh — the shell's script language

`msh` (issue 094) is the shell's own small language, for start-up sequences, tests, repeated tasks and recipes. It is
not a POSIX shell. It has no quoting puzzles, no global environment and no ambient authority. Commands return
**results**, as Marain does (RFC 001 §7.5), and a script declares what it needs.

The interpreter is `mind::script` (libmind, with the `alloc` feature). It is a parser to a syntax tree, then a
tree-walking evaluator with a step budget. The shell is its host: commands, typed answers from the system, output.

[Русская версия](msh_RU.md)

## Running

| How | What |
|---|---|
| `msh data/sum.msh a b` | runs a script file; `args` is `["a", "b"]` |
| `data/sum.msh a b` | the same: a name ending in `.msh` is a script |
| `msh -c "let x = 1; print(x)"` | one line, with the prompt's variables |
| `msh --check data/sum.msh` | parses, and names the functions that do not exist |
| `let`, `if`, `for`, `while`, `fn`, `try`, `name = …`, `name(…)` typed at the prompt | a statement; the console's variables and functions stay |

A script stops at its first unhandled error with `SCRIPT FAILED: reason (LINE n)`. Esc or Ctrl+C on either keyboard,
or Ctrl+Z on the serial line, stops it with `SCRIPT STOPPED: stopped`. A script that runs past its step budget (ten
million steps) stops the same way.

## A script

```
#!msh
requires: files
# The files on ram:, their sizes summed into ram:summary.txt; a missing file handled.
let total = 0
let names = []
for f in files("ram:")? {
    if f.dir { continue }
    total = total + f.size
    names = push(names, f.name)
}
write ram:summary.txt "{len(names)} files, {total} bytes: {join(names, " ")}"
cat ram:missing.txt or { print("handled: {error}") }
print("done: {total}")
```

## Lines

- One statement per line, or several separated by `;`. `#` starts a comment.
- A line that starts with a name is a **command**, unless the name is a keyword or is followed by `(` (a call) or `=`
  (an assignment).
- **Command words.** Words are separated by spaces. `"a quoted word"` keeps its spaces. `{expr}` inside a word or a
  quoted word puts in the expression's value. `\` takes the next character as it is.
- **End of a command.** The line, `;` or `}` ends it. A last word `or` followed by a handler handles its failure.
- **Commands.** A command is one of the shell's own (`ls`, `write`, `ping`, …) or a program. It waits for a console
  program to end. It returns `ok(nil)`, or `err(reason)` when it printed an `ERROR:` line, or when its program ended badly:
  an exit code other than 0 (`grep exited with 1`: nothing matched), a kill, a fault (issue 166).
- **Statements:**

| Statement | |
|---|---|
| `let name = expr` | a variable in the current block |
| `name = expr` | changes a variable `let` made |
| `if cond { … } else if cond { … } else { … }` | `cond` must be `true` or `false` |
| `while cond { … }`, `for item in list { … }` | `break`, `continue` |
| `fn name(a, b) { … return value }` | a function sees its parameters and the script's outermost variables |
| `try { … } catch e { … }` | `e` is the failure's reason |

## Values

| Value | Written |
|---|---|
| nothing | `nil` |
| booleans | `true`, `false` |
| integers | `42` (64-bit, overflow fails) |
| strings | `"text {expr} \n \t \" \\ \{"` |
| lists | `[1, "two", [3]]`, indexed `list[0]`, `list[-1]` |
| records | `{name: "x", size: 3}`, field `r.name`, `r["name"]` |
| results | `ok(value)`, `err(reason)` |

Operators, from the lowest:
- `or` (a handler);
- `||`, `&&` (booleans only);
- `==`, `!=`, `<`, `<=`, `>`, `>=` (integers or strings);
- `+`, `-` (`+` also joins strings and lists);
- `*`, `/`, `%`;
- `!`, `-`;
- the postfixes `.field`, `[index]`, `?`.

## Results

Every command, and every function that can fail, returns `ok(value)` or `err(reason)`. **Nothing continues silently
after an error**:
- A statement whose value is `err(r)` fails the script there: a command line, a call, `err("x")`.
- `expr?` is the value inside `ok(v)`; on `err(r)` it fails.
- `expr or { … }` or `expr or other` handles a failure or an `err(r)`: the handler runs with `error` set to `r`, and its
  last value is the value.
- `try { … } catch e { … }` handles any failure in a block.
- `fail(reason)` fails at once.
- A result kept in a variable (`let r = capture("ps")`) is a value. Using it as a string or a number fails with a
  reminder to take its value with `?` or `or`; `is_ok(r)` and `is_err(r)` test it.

## Functions

- **Built in:** `print(…)`, `len`, `str`, `int` (an `err` when not a number), `type`, `lines`, `words`,
  `split(text, sep)`, `join(list, sep)`, `trim`, `upper`, `lower`, `replace(text, from, to)`, `contains` (a string,
  a list, or a record's field), `starts_with`, `ends_with`, `match(text, pattern)` (the patterns of `grep`: `.` `*`
  `[a-z]` `^` `$`), `push(list, item)` (a new list), `range(a, b)`, `keys(record)`, `ok`, `err`, `fail`, `is_ok`,
  `is_err`, `unwrap`.
- **From the shell:**

| Function | Returns |
|---|---|
| `capture("command line")` | `ok(text)`: what the command printed, kept off the screen; or `err(reason)` |
| `ps()` | `ok([{pid, name, state, cpu, service, focus, console}, …])` |
| `services()` | `ok([name, …])`: the services running |
| `files(dir)` | `ok([{name, size, dir}, …])` |
| `glob("ram:*.txt")` | `ok([path, …])`: the names in a directory that match a mask (`*`, `?`, any case) |
| `log(text)` | a line in the system log (needs `log`) |
| `sleep(ms)` | waits (Esc or Ctrl+Z stops it) |
| `now()` | the uptime in milliseconds |

## Authority

- A script runs with the authority of the shell session that starts it, **but no more than it declares**. The
  declaration is its first line after `#!msh` and comments: `requires: files network`, in the words of
  `mind::request!`:
  - `files`: `write`, `mkdir`, `rm`, `mv`, `sync`, `screenshot`;
  - `network`: `ping`, `nslookup`, `fetch`, `https`, `net`, `netrevoke`, `ip`, `tls`;
  - `log`: `logger`, `log()`;
  - `lifecycle`: `kill`, `budget`, `stop`, `reboot`.

  Without its word a command fails: `ping needs requires: network in the script`.
- **Programs a script starts** get, of what they ask for, only what the script declared (`sysinfo`, `file`, `files`,
  `log`, `lifecycle`, `network`, `authority`, `display`, `window-manager`, `gpio`, `camera`, `blockstore`, `firmware`,
  `tls`); they run without the rest. `tls` lends the shell's TLS client only together with a flow grant (`network`). A program asking for `camera` or `firmware` still gets it only if the user says yes when
  the shell asks.
- **A script from outside the boot disk** (`ram:`, a USB disk) asks once before it runs:
  `SCRIPT ram:w.msh REQUIRES files. ALLOW? (Y/N)`.
- **What the user types** at the prompt (and `msh -c`) has the session's authority.

## Not yet

- **Foreground programs:** a script waits for one to end or to leave the foreground. Keys typed on the serial line do not reach it while the script waits.
- **`ip()`:** the addresses through a typed answer; for later.
