# issue rubash#700 - read builtin on /dev/stdin.
# The original face (hang forever) is fixed; the residual is the
# pipeline-context empty read - /dev/stdin and <&0 inside a pipe stage
# see EOF where fd 0 has data (tracked as rubash#938, expected `diverges`
# until that lands).

# fixed face: /dev/stdin against a regular-file stdin works.
printf 'FILE\n' > sf
{ read -r y </dev/stdin; echo "file=[$y]"; } < sf

# residual faces (rubash#938): same read, fd 0 is a pipe.
printf 'SIN\n' | { read -r z </dev/stdin; echo "pipe=[$z]"; }
printf 'DUP\n' | { read -r w <&0; echo "dup0=[$w]"; }

# control: plain read from the same pipe is correct on both shells.
printf 'PLAIN\n' | { read -r v; echo "plain=[$v]"; }
