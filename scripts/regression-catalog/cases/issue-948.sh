# rubash exports THIS_SH pointing at itself; the GNU arm has no harness
# variable, so fall back to `bash` (the same name the runner used to
# launch this probe — the oracle resolves identically).
: "${THIS_SH:=bash}"
"$THIS_SH" -c 'echo "__RUBASH_CSB1_41;"'
"$THIS_SH" -c 'echo "x41;y"'
"$THIS_SH" -c 'echo "x41;y" "done"'
