# issue rubash#871 - printf %f/%e/%g inf/nan word-prefix subclass.
# inf/infinity/nan are recognised; the prefix subclass (infin/nann/infx/
# nanx) print the right value AND emit `invalid number` + rc 1 - the
# face is byte-identical to GNU bash incl. the diagnostic lines and rc,
# verified differentially; the manifest row is must-match (promoted per
# PR #944 review).  If this ever diverges again, it is a regression.
for w in inf infinity infin nan nanx infx nann; do
  printf "[%f]\n" "$w" 2>d; echo "$w rc=$?"
  [ -s d ] && echo "$w diag" || echo "$w silent"
done
