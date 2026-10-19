# rubash#815 profiler fixture script: functions, recursion via command
# substitution, builtin/external/subshell faces in one run.
fib() {
  if [ "$1" -le 2 ]; then
    echo 1
  else
    echo $(( $(fib $(($1-1))) + $(fib $(($1-2))) ))
  fi
}
wrap() {
  local r
  r=$(fib 5)
  echo "fib5=$r"
  ls . >/dev/null 2>&1
  ( echo sub )
}
wrap .
