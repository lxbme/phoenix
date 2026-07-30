# SeqLang

A sequencial programing language

## usage

	cargo build --release
	./target/release/phoenix program.sl

Only the program's own output goes to stdout, so `phoenix program.sl > out.txt` captures exactly that; diagnostics, `--trace` and `-v` go to stderr. A successful run says nothing else.

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
	
comments - []

	[this is a comment]

define variable - var:
	
	var x [define x and allocate memory (x will be init to 0.0)]
	
	[!! if x has been defined, this will shadow the old one]
	[!! all the variables is global]
	
read variable:

	x [this will push the value of x to stack]

store number to variable - !

	x ! [store top of stack to x and consume it]
	
	
define function - def func_name { body }

	def foo { var x x ! var _ _ ! var z z ! z x + x ! }
	[add first and third value in stack and save result to x 
	 and discard second value]
	 
	[it is recommened to write it like:]
	def foo[3 -> 0] {funcBody}
	[to clearify number of parameters and returns]
	
	[recursion is **NOT** allowed]

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

	5.0 3.0 1.0 $foo[3->1]
	[the stack will change to |6.0|]

conditioning - if {} else {}

	x 1 = if { 1 x + } else { 1 x - }
	["if" will check the top of the stack is positive or not and consume it]
	["else" statement is mandatory]

loop - dow {}

	1 x ! dow { x y + y ! 1 x + x ! 100 x < }
	[dow is do-while, it will check if the top of stake is positive and consume it]

output - print printa
	
	1 print
	[will print top of stack and consume it]
	
	32 printa
	[try to convert top to char in ascii and print-consume it]
