# Examples

Programs that teach the language, meant to be read in order. Each one runs on its own and prints something, and the comments inside it are the explanation — there is no separate tutorial to keep in sync.

	cargo build --release
	./target/release/phoenix examples/000_hello_world.phx

The numbering leaves gaps so something can be added between two existing files without renumbering everything after it.

| | |
|---|---|
| `000_hello_world` | there are no strings; `printa` writes one character code |
| `010_the_stack` | values pile up, `print` takes one off |
| `020_operand_order` | the top of the stack is the **left** operand |
| `030_variables` | `var`, `x !`, and why the `!` has to sit next to a name |
| `040_conditionals` | `if` always needs its `else`; comparisons push 1 or 0 |
| `050_loops` | `dow` runs until its condition holds — the opposite of `while` |
| `060_functions` | arguments and results travel on the stack |
| `070_locals_and_recursion` | a `var` inside a `def` belongs to the call |
| `080_arrays` | `arr`, `@name`, and the index on top |
| `090_input` | `read` / `reada` and the end-of-input tests that pair with them |
| `100_sieve` | all of the above, and nothing new |

Only `090_input` reads anything; give it numbers on standard input, or none at all — it copes either way.

	echo "3 1 4 1 5" | ./target/release/phoenix examples/090_input.phx

Every file states the output it should produce, and every one of them was checked against the interpreter rather than written from memory. `070` is the one exception to a clean run: it prints a shadowing warning on purpose, because the trap it warns about is the point of that example.

If you read only one of these, read `020`. Reversed operand order is the single thing most likely to make a correct-looking program compute the wrong answer.
