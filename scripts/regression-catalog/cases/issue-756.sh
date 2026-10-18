# 756.1 - `history -w` with HISTFILE unset must NOT terminate the shell
# (rubash#756, closed 2026-10-17): the severe half was silent shell death;
# the fixed shell reaches the next line and exits 0. The stderr WORDING
# ("parameter null or not set" vs "filename not specified") still diverges
# and stays with the diagnostic-message family (#551), so this case silences
# stderr and pins the surviving surface on stdout + rc.
unset HISTFILE
history -w 2>/dev/null
echo "reached the next line"
