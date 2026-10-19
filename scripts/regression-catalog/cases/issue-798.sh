# 798.1 - `exit` inside a sourced file terminates execution at the exit with
# the given status (rubash#798, fixed in the #805 wave; the harvest asset ran
# a 47-cipher failure loop through the watchdog because exit-in-source was
# swallowed). Sourcing in a subshell keeps the harness's completeness marker
# reachable while pinning the face: the subshell must stop at the exit
# (AFTER-EXIT/UNREACHED never printed) and report rc 1 to the parent.
cat > exit1.sh <<'EOS'
echo BEFORE-EXIT
exit 1
echo AFTER-EXIT
EOS
( source ./exit1.sh )
echo "subshell-rc=$?"
echo PARENT-CONTINUES
