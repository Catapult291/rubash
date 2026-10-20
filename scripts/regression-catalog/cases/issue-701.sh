# issue rubash#701 - read builtin: /dev/fd/N reads and the
# EOF-without-newline return status under compound/fd redirection.

# 701.1 - read resolves /dev/fd/N to the open fd.
printf 'DATA\n' > f; exec 6<f
read -r z </dev/fd/6; echo "fd6=[$z]"

# 701.2 - EOF-without-newline returns 1 under compound redirection.
printf 'abc' > g
{ IFS= read -r l; echo "grp-rc=$?"; } < g

# ... and on a duplicated fd.
exec 9< g; IFS= read -r l2 <&9; echo "dup-rc=$? l2=[$l2]"

# the classic while-read off-by-one on a last line without newline.
printf 'abc' > h
n=0; while IFS= read -r l; do n=$((n+1)); done < h; echo "n1=$n"
printf 'aaa\nbbb' > i
m=0; while IFS= read -r l; do m=$((m+1)); done < i; echo "m2=$m"
