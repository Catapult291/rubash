# issue rubash#830 - popd default pop must chdir, not merely rewrite PWD.
# `pushd -n ./sub; popd` leaves the process cwd inside ./sub under GNU.
mkdir -p sub
root=$PWD
pushd -n ./sub >/dev/null 2>&1
popd >/dev/null 2>&1
echo "PWD-tail=${PWD##*/}"
# Physical-cwd evidence, tail segment only: the harness stages each arm in
# its own dir (out/issue-830/{arm,gnu}/), so a full-path `pwd -P` can never
# be equal across the two arms (gnu spells /mnt/<drive>/..., the arm
# <drive>:\...) and the normaliser legitimately cannot reconcile them.
# Whether chdir *happened* is already pinned by PWD-tail and phys= below.
basename "$(pwd -P)"
# external truth: a cwd-relative file created now must land inside ./sub.
: > probe_cwd
[ -f "$root/sub/probe_cwd" ] && echo "phys=sub" || echo "phys=not-sub"
