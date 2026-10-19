# 797.1 - sourcing a bash-preexec-shaped script (DEBUG-trap assembly) must
# leave the shell able to run the commands after the source (rubash#797,
# fixed in the #805 wave). Pre-fix the REPL lost execution on the first
# command after such a source; the non-interactive proxy pins the same
# assembly path: the trap installs, the sourced body ends, and the two
# trailing commands must still run.
cat > preexec-like.sh <<'EOS'
__preexec() { true; }
trap '__preexec' DEBUG
EOS
source ./preexec-like.sh
echo ALIVE-1
echo ALIVE-2
