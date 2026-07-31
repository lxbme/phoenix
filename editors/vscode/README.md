# Phoenix for VS Code

Syntax highlighting for the Phoenix language, for files ending in `.phx`.

This is a declarative extension: a TextMate grammar and a language configuration, and nothing else. There is no extension code, no dependencies and no build step.

## Installing

Link it into the extensions folder and restart VS Code:

	ln -s ~/Projects/phoenix/editors/vscode ~/.vscode/extensions/lxbme.phoenix-lang-0.1.0

A symlink means edits to the grammar take effect on the next window reload, with nothing to rebuild. To produce a `.vsix` instead, `npm i -g @vscode/vsce` and run `vsce package` in this folder.

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
