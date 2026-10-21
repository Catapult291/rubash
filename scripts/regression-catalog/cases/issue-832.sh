# issue rubash#832 - the $(...) fast-path detour must not consult CWD
# contents: a 0-byte file named `s` in CWD stripped the quotes from
# $(echo 's').  The same probes run in a dir with and without `s`;
# both blocks must print identical lines.
p() { printf '<%s>' "$@"; printf '\n'; }
mkdir -p with_s without_s
( cd with_s; : > s
  x=$(echo 's'); echo "x=[$x]"
  p "$(echo 's')"
  p $(echo 's')
  y=$(echo 's s'); echo "y=[$y]" )
( cd without_s
  x=$(echo 's'); echo "x=[$x]"
  p "$(echo 's')"
  p $(echo 's') )
