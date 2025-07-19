[ a b c -- b c a ]
1.0 2 3.0
def rot[ 3 -> 3 ] {
    var c c !   [c = 栈顶]
    var b b !   [b = 新的栈顶]
    var a a !   [a = 再新的栈顶]
    b           [把 b 压栈]
    c           [把 c 压栈]
    a           [把 a 压栈]
}

$rot

printa
print

+ - * / > < = ~
