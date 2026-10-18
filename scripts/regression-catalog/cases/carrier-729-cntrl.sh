# 729.2/729.3 — [[:cntrl:]]/[[:space:]] must classify carried raw bytes by
# the byte they carry (GNU sm_loop.c/subst.c walk unsigned char buffers).
for b in 001 002 003 004 005 011 012 013 014 015 016 017 020 021 022 023 024 025 026 027 030 031 032 033 034 035 036 037 177; do
  x=$(printf "\\$b")
  [ -z "$x" ] && continue
  [[ $x == *[[:cntrl:]]* ]]; c=$?
  [[ $x == *[[:print:]]* ]]; p=$?
  printf 'byte %s cntrl=%s print=%s\n' "$b" "$c" "$p"
done
x=$'\x0b'; [[ $x == *[[:space:]]* ]]; echo "space-vt=$?"
v=$'a\x01c'
echo "rem-lo=${v#*$'\x01'}"
echo "rem-hi=${v%$'\x01'*}"
echo "patsub=${v//[[:cntrl:]]/X}"
printf 'a\033b\n' | tr '[:cntrl:]' X
