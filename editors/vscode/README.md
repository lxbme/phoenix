# Phoenix for VS Code

Syntax highlighting and compiler diagnostics for the Phoenix language, for files ending in `.phx`.

There are no dependencies and no build step. The grammar is declarative, and the diagnostics are plain JavaScript against the `vscode` module the editor already provides — nothing to `npm install`, nothing to compile, so a symlink is a complete installation.

## Installing

Link it into the extensions folder and restart VS Code:

	ln -s ~/Projects/phoenix/editors/vscode ~/.vscode/extensions/lxbme.phoenix-lang-0.2.0

A symlink means edits to the grammar take effect on the next window reload, with nothing to rebuild. To produce a `.vsix` instead, `npm i -g @vscode/vsce` and run `vsce package` in this folder.

## Diagnostics

On opening and on saving a `.phx` file, the extension runs

	phoenix -c --message-format=json <file>

and turns each line of the output into a squiggle. `-c` means analyse only: **the extension never runs the program it is looking at**, so opening a file has no side effects.

A run-time call trace arrives as an error followed by `note` frames. Those are not separate problems, so they become the error's *related information* — one squiggle, with the chain of calls listed under it as clickable links.

Two settings, both under `phoenix`:

- `phoenix.path` — where the binary is. Empty means try `phoenix` on `PATH`, then `target/release/phoenix` and `target/debug/phoenix` under the workspace folder. Setting it explicitly disables the fallbacks, so a typo is reported rather than quietly worked around.
- `phoenix.check.enable` — turn the checking off and keep the colours.

There is also a **Phoenix: Check File** command for re-running it by hand.

### Why on save and not while typing

Checking unsaved text would mean piping it through `phoenix -`, and the compiler quite rightly warns that a program read from stdin leaves no stdin for the program to read. Every program using `read` would then carry a permanent false warning. Saving is cheap enough that the delay is hard to notice.

## What it colours

Keywords, operators, numbers and comments, plus the things that carry most of the meaning in a postfix language:

- `$name` as a call and `@name` as an array, so the two sigils read differently from a plain variable
- the name being declared in `def f`, `var x` and `arr a 16`
- the `!` of a store, both `x !` and `v i @a !`, as an assignment rather than a bare operator

It also marks three mistakes red before the compiler is ever run:

- a number literal with more than one dot, and a lone `.`
- any character the lexer does not accept
- a `!` with nothing to store into

The second of those is worth more than it sounds. Comments do not nest, so in

	[ halve[1 -> 1] : one in, one out ]

the comment ends at the *first* `]` and the rest of the line is code. The grammar reproduces exactly that, so the leftover `:` and `]` show up red.

## What it cannot colour

A TextMate grammar is regular expressions over one line at a time; it has no idea what any name means. In particular it cannot show the one thing that is easiest to misread in this language — that a name is local only *below* its own declaration:

	def f {
		x print          [ the global x ]
		var x  1 x !
		x print          [ the local x  ]
	}

Both `x` are coloured the same. Telling them apart needs the compiler's own scope resolution, which would mean semantic tokens or a language server.

Two smaller corners:

- The compiler requires the `!` of a store to be the next *token*, not the next character, so `x [note] !` is a legal store. The grammar only sees characters, and flags that `!`.
- A declaration split across a newline (`var` at the end of one line, the name at the start of the next) is legal, but a TextMate match rule cannot span lines, so only the keyword is coloured.

## Keeping it in sync

The keyword list here mirrors `src/lexer.rs`. When a keyword or an operator character is added there, add it to `syntaxes/phoenix.tmLanguage.json` in the same commit — that is why this lives in the language's own repository rather than in one of its own.

`diagnostics.js` deliberately does not import `vscode`, so the mapping from the compiler's JSON to editor positions can be exercised under plain `node` against real compiler output. `extension.js` holds everything that needs the editor to exist.

An older binary that does not understand `--message-format` exits with `EX_USAGE`, which the extension recognises and reports as "rebuild it" rather than leaving the file silently unchecked.
