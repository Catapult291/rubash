# rubash#876: mark_quoted_ifs_from_raw recorded quoted-IFS positions as BYTE
# offsets while the apply loop counted CHARS — a multi-byte char before a
# quoted IFS blank shifted the record past the counter and the blank split.
# Each face below carries a multi-byte lead before a quoted IFS char that
# must stay protected while the trailing unquoted expansion still splits.
p() { printf '<%s>' "$@"; printf '\n'; }
w=W
IFS=' :'
p "é a"$w
p "中文 a"$w
IFS=:
p "é:a"$w
IFS=' :'
p 'é a'$w
p é\ a$w
p "é: a"$w
w='中 x'
p "é q"$w
unset w
p "é a"$w
