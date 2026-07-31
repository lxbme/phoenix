# Phoenix Language

A sequencial programing language

## usage

	cargo build --release
	./target/release/phoenix program.phx

Only the program's own output goes to stdout, so `phoenix program.phx > out.txt` captures exactly that; diagnostics, `--trace` and `-v` go to stderr. A successful run says nothing else.

	phoenix [OPTIONS] <FILE>
	phoenix [OPTIONS] -            read the program from stdin

	-c, --check                    analyse only, do not run
	    --dump-tokens              print the token stream and exit
	    --dump-opcodes             print the compiled program and exit
	    --trace                    print each instruction and the stack
	-W, --deny-warnings            treat warnings as errors
	-v, --verbose                  print progress information
	-h, --help                     print this message
	-V, --version                  print the version

Exit codes: `0` success, `1` the program did not compile, `2` the program failed at run time, `64` bad command line.

## grammar

variables' type: float-64 (f64 in rust)

basic operator:
	
	+ [add]
	- [minus]
	* [time]
	/ [devide]
	
	= [equal]
	~ [not equal]
	> [greater]
	< [smaller]
	[1.0 will be pushed to stack if compare success, 0.0 for unsuccess]

	[!! operand order: the TOP of the stack is the LEFT operand]
	[to write the infix expression "a op b", push b first, then a]

	10 3 -      [is 3 - 10, so -7]
	2 10 /      [is 10 / 2, so 5]
	10 3 <      [is 3 < 10, so 1.0]
	1 x -       [is x - 1]
	100 x <     [is x < 100]

	[this is the opposite of most postfix languages, and it is deliberate:]
	[it lets a condition be written in the order it reads, as in "100 x <"]

comments - []

	[this is a comment]

	[!! an unclosed "[" comments out the rest of the file]

define variable - var:
	
	var x [define x and allocate memory (x will be init to 0.0)]
	
	[!! at the top level a variable is global; inside a "def" it is local]
	[to that call - see "scope" below]
	[!! re-declaring resets the value to 0.0, it does not shadow]

	var x  5 x !  var x  x print      [prints 0, not 5]

	[!! "var" runs at run time, so a variable must be declared before the]
	[first time control reaches a use of it - not merely earlier in the file]
	
read variable:

	x [this will push the value of x to stack]

store number to variable - !

	x ! [store top of stack to x and consume it]

	[!! "!" must directly follow a variable name]
	[the target is resolved at compile time, not taken from the stack, so]
	[anything else before "!" is a compile error - including "var x !",]
	[where "var" has already consumed the name. write "var x  x !"]

define array - arr name length:

	arr board 16    [16 elements, every one init to 0.0]

	[the length is a literal whole number between 1 and 16777216. it is]
	[read at compile time, so it cannot be an expression or a variable]

read and write an element - @name:

	i @board            [push board[i], consuming i]
	v i @board !        [store v into board[i], consuming both]

	[the shape mirrors the scalar case exactly - the only difference is]
	[that the index comes off the stack:]

	[  scalar     x          v x !      ]
	[  array      i @x       v i @x !   ]

	[!! push the value first and the index second, so the index is on top]
	[!! "@" must directly follow nothing and be followed by the name, and]
	[the "!" of an element store must directly follow the name]

	[!! an index must be a whole number in range. a fractional, negative,]
	[out of range or NaN index is a run-time error, never rounded - a]
	[silently floored index reads a neighbour's element instead]

	[!! arrays and variables share one namespace. "arr x 4" after "var x"]
	[is an error, and using one as the other is caught at compile time]

	[!! like "var", "arr" runs at run time and re-declaring zeroes it]
	[!! also like "var", an "arr" inside a "def" is local to the call]

	arr a 5
	var i  0 i !
	dow { i 10 *  i @a !   1 i + i !   4 i > }    [a = 0 10 20 30 40]

	
define function - def func_name { body }

	def foo { var x x ! var _ _ ! var z z ! z x + x ! }
	[add first and third value in stack and save result to x 
	 and discard second value]
	 
	[it is recommened to write it like:]
	def foo[3 -> 0] {funcBody}
	[to clearify number of parameters and returns]

	[!! "[3 -> 0]" is only a comment - nothing checks it yet]

scope - what a name refers to:

	[a "var" or "arr" written inside a "def" declares storage belonging to]
	[that call, not to the program. every call gets its own copy, so a]
	[function is reentrant and recursion works]

	def fact {
		var n  n !
		2 n < if { 1 } else { 1 n - $fact  n * }
	}
	5 $fact print      [prints 120]

	[!! a name is local only BELOW its own declaration. above it, the same]
	[name still means the global. this follows the rule that declarations]
	[run where they are written:]

	var x  9 x !
	def f {
		x print          [the global x, so 9]
		var x  1 x !
		x print          [the local x, so 1]
	}
	$f  x print          [the global is untouched, so 9 again]

	[that is a trap, so declaring a local that hides a global warns]

	[!! a use above its own declaration that has no global to fall back on]
	[is an error, and says so:]
	[   error: undefined variable `y`                                    ]
	[   = note: this function declares that name further down            ]

	[!! a declaration that control never reaches is an error to use, the]
	[same as for a global - "var" inside an if branch that was not taken]
	[does not give you the variable]

	[!! calls are limited to 1024 deep, so a runaway recursion says so]
	[instead of exhausting memory]
	[it compiles and runs, but every variable is global, so a nested call]
	[overwrites the caller's own variables. there are no call frames yet]

	[!! function names are global]
	[a def is always global and always defined, no matter where it is written]
	[the body is compiled in place, with a jump in front of it, so execution]
	[steps over the body instead of falling into it]

	0 if { def g { 7 print } } else { 99 print } $g
	[prints "997"]
	[the if branch is never taken, yet g is still callable]

	1 if { def g { 7 print } 42 print } else { 99 print }
	[prints "42"]
	[the block behaves as if the def were not there at all]

	[!! write def at top level only]
	[putting a def inside if / else / dow does not make it conditional]
	[def cannot be nested - a def inside another def is a compile error]

call function - $function_name

	5.0 3.0 1.0 $foo[3 -> 0]
	[foo consumes all three values and leaves the stack empty]
	[the result, 6.0, ends up in the variable x that foo stored into]

	[!! a function returns values by leaving them on the stack]
	[a function that keeps its result in a global variable, like foo above,]
	[returns nothing - the caller reads the variable instead]

conditioning - if {} else {}

	x 1 = if { 1 x + } else { 1 x - }
	["if" will check the top of the stack is positive or not and consume it]
	["else" statement is mandatory]

loop - dow {}

	[dow is do-while: the body always runs at least once, then the top of]
	[the stack is consumed and checked]

	[!! the condition is an EXIT condition, not a continue condition]
	[the loop repeats while it is 0.0 or negative, and stops once it is]
	[positive. read "dow { body cond }" as "do body until cond"]

	var x  var y
	1 x ! dow { x y + y ! 1 x + x ! 99 x > }
	[sums 1..99 into y, because it stops once x > 99]

	[a pre-tested loop - the equivalent of "while cond" - is written as]
	[   <cond> if { dow { <body> <cond> 0 = } } else { }   ]
	[the "0 =" turns the continue condition into an exit condition]

output - print printa
	
	1 print
	[will print top of stack and consume it]
	
	32 printa
	[try to convert top to char in ascii and print-consume it]

input - read reada eof eofa

	read
	[read one whitespace-delimited number from stdin and push it]
	[running out of input here is a runtime error - use "eof" to avoid it]

	reada
	[read one byte from stdin and push its value]
	[pushes -1 when the input is exhausted]
	[byte oriented, to match printa, which is ascii only]

	eof
	[push 1.0 when no number remains, 0.0 otherwise]
	[skips the whitespace between numbers, so it pairs with "read"]

	eofa
	[push 1.0 when no byte remains, 0.0 otherwise]
	[does not skip anything, so it pairs with "reada"]

	[!! "eof" discards whitespace while looking ahead]
	[mixing it with "reada" therefore loses the whitespace - use "eofa" there]

	[since dow is do-while, a pre-tested loop is written as]
	[   <cond> if { dow { <body> <cond> 0 = } } else { }   ]

	[sum every number on the input:]
	var sum
	0 sum !
	eof if { } else { dow { read sum + sum ! eof } }
	sum print

	[echo the input unchanged:]
	eofa if { } else { dow { reada printa eofa } }

	[!! "phoenix -" reads the program itself from stdin, leaving no input]
	[to read; the compiler warns about it. pass the program as a file instead]
