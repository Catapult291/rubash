# issue rubash#840 - marker output-boundary residuals (three faces from
# the #824 fix audit; open).  The literals below are real PUA codepoints
# typed into the source, not carrier bytes produced by the engine.
o() { od -An -tx1 | tr -d ' \n'; printf '\n'; }

# face1: declare -p of an element holding a typed PUA codepoint leaks the
# E400 marker prefix into the rendered output.
a=($'')
printf 'f1='; declare -p a | o

# face2: user-typed lookalike text __RUBASH_CSB1_<hex>; must pass through
# printf/echo verbatim - the output boundary must not re-decode it.
printf 'f2=[%s]\n' '__RUBASH_CSB1_41;'
printf 'f2b=[%s]\n' 'pre__RUBASH_CSB1_5c;post'
echo "f2c=__RUBASH_CSB1_21;"

# face3: typed PUA via an ANSI-C assignment must survive end to end
# (matches GNU on the 2026-10-21 arm check - kept as the in-case
# parity control).
v=$''
printf 'f3='; printf '%s' "$v" | o
