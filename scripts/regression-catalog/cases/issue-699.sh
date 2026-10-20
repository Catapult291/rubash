# issue rubash#699 - redirection & fd family (five faces, fixed).
# Diagnostics whose wording is not pinned are routed to files and only
# emptiness is asserted; the faces below pin routing/order/status, not text.

# 699.1 - `2>&1 >file` order: fd 2 binds to the ORIGINAL stdout before
# fd 1 moves to the file.  stderr must show ERR, the file only OUT.
{ echo OUT; echo ERR >&2; } 2>&1 >f
echo "r11-file=[$(<f)]"
f2() { echo OUT2; echo ERR2 >&2; }
f2 2>&1 >/dev/null

# 699.2 - a builtin's write-error diagnostic honours the command's own
# redirected stderr (fd 4 is read-only; echo X >&4 fails to write).
printf 'ro\n' > r; exec 4< r
echo X >&4 2>e; echo "r12-rc=$?"
if [ -s e ]; then echo "r12=e-nonempty"; else echo "r12=e-empty"; fi

# 699.3 - closed stderr: no diagnostic escapes and the applet status
# survives (`ls` ENOENT is 2, `cat` ENOENT is 1 under GNU).
ls /nonexistent_699 2>&-; echo "r13-ls=$?"
cat /nonexistent_699 2>&-; echo "r13-cat=$?"

# 699.4 - a subshell's cd diagnostic honours the subshell's stderr
# redirect (only emptiness asserted, not the message text).
( cd /no_such_699 ) 2>e2
if [ -s e2 ]; then echo "r14=e2-nonempty"; else echo "r14=e2-empty"; fi

# 699.5 - bare-cat self-redirect: `cat c > c` truncates (and GNU cat
# refuses input==output), `cat c >> c` leaves the content alone.
printf 'hello\nworld\n' > c; cat c > c 2>/dev/null
mapfile -t A < c; echo "r15-trunc=${#A[@]}"
printf 'hello\nworld\n' > c2; cat c2 >> c2 2>/dev/null
mapfile -t B < c2; echo "r15-append=${#B[@]}"
