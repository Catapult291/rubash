# rubash#868: mark_quoted_ifs_from_raw was carrier-blind — a raw word whose
# cook produces markers.rs carriers ($'...' ANSI-C decode, escaped quotes,
# escaped IFS blanks/pattern chars) simulated unequal to the cooked word on
# every char and the per-char IFS marking silently degraded to the coarse
# fallback. Each face below carries a quoted/decoded IFS char that must be
# protected while the trailing unquoted expansion still splits.
p() { printf '<%s>' "$@"; printf '\n'; }
w=x:y
W='c d'
IFS=:
p "a:b"$w
p "a:b"$'x'$w
IFS=$' \t\n:'
p "a b"$'x'$W
IFS=:
p "a:b"\'z\'$w
p "a:b"$"x"$w
p a\:b$w
p 'a:b'$w
unset w
p "a:b"$w
