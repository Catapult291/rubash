# issue rubash#856 - an isolated literal U+E000 must pass the output
# boundary verbatim (it is not a raw-byte carrier without a pairing E0xx
# successor).  The literals below are real PUA codepoints in the source;
# the $'\uE000' escape spelling takes a different path and already
# matches GNU (f0 is the in-case control).
o() { od -An -tx1 | tr -d ' \n'; printf '\n'; }
v=$'a\uE000b';     printf 'f0='; printf '%s' "$v" | o
v=$'ab';       printf 'f1='; printf '%s' "$v" | o
v=$'ab';   printf 'f2='; printf '%s' "$v" | o
v=$'ab';   printf 'f3='; printf '%s' "$v" | o
v=$'ab';       printf 'f4='; printf '%s' "${v//a/ZZ}" | o
v=$'ab';       printf 'f5='; printf '%s' $v | o
