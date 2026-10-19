k=$'x\ty'
declare -A A; ((A[$'x\ty']=1)); printf 'A=[%s]\n' "${A[$k]}"
printf 'Akeys='; printf '%s' "${!A[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
printf 'Aarith=%s\n' "$((A[$'x\ty']))"
declare -A D; ((D[$'a\\b']=2)); printf 'Dkeys='; printf '%s' "${!D[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A B; let "B[$'x\ty']=1"; printf 'Bkeys='; printf '%s' "${!B[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
let "v=B[$'x\ty']"; printf 'Bself=[%s]\n' "$v"
declare -A O; let "O[$'a\\b']=2"; printf 'Okeys='; printf '%s' "${!O[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A E3c; ((E3c["pre$'x'post"]=1)); printf 'E3c='; printf '%s' "${!E3c[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
declare -A E1b; E1b['a$'b'c']=1; printf 'E1b='; printf '%s' "${!E1b[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
