# issue rubash#700 - read builtin on /dev/stdin.
# The original face (hang forever) is fixed, and so is the residual
# pipeline-context empty read (rubash#938): a fd-0 self-dup (`<&0`,
# `</dev/stdin`, `/dev/fd/0`, `/proc/self/fd/0`, even the `<&0-` move
# form) is a GNU no-op under redir.c:1128 `redir_fd != redirector`, so
# read must serve the ordinary fd-0 transports — the stage's
# FUNCTION_STDIN mirror included — not an fd-table-only lookup.

# fixed face: /dev/stdin against a regular-file stdin works.
printf 'FILE\n' > sf
{ read -r y </dev/stdin; echo "file=[$y]"; } < sf

# fixed residual faces (rubash#938): same read, fd 0 is a pipe.
printf 'SIN\n' | { read -r z </dev/stdin; echo "pipe=[$z]"; }
printf 'DUP\n' | { read -r w <&0; echo "dup0=[$w]"; }

# control: plain read from the same pipe is correct on both shells.
printf 'PLAIN\n' | { read -r v; echo "plain=[$v]"; }
