# issue rubash#871 - printf %f/%e/%g inf/nan word-prefix subclass.
# inf/infinity/nan are recognised; the prefix subclass (infin/nann/infx/
# nanx) prints the right value but still emits `invalid number` and
# returns 1 - residual of the #869 fix, kept `diverges`.
for w in inf infinity infin nan nanx infx nann; do
  printf "[%f]\n" "$w" 2>d; echo "$w rc=$?"
  [ -s d ] && echo "$w diag" || echo "$w silent"
done
