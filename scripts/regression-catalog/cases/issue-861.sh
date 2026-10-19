# issue rubash#861 - kvlist self-read miss (write parity, read miss).
#
# The strict `name=( [key]=value )` path cooked a kept `$'...'` spelling's
# quotes into E010/E011 data markers and stored them IN the key, so the
# stored cell was `pre$<E010>x<E010>post` while every reader resolved the
# plain spelling: the same-spelling read missed, the listing/rendering
# channels decoded the markers (byte-identical od), and a clean-spelling
# overwrite added a SECOND cell. GNU: one clean key, all reads hit.
#
# GNU bash 5.3.0 baselines, byte-verified probes 2026-10-19.
declare -A K; K=( ["pre$'x'post"]=v )
printf 'k1keys='; printf '%s' "${!K[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
printf 'k1read=[%s]\n' "${K["pre$'x'post"]}"
declare -A K2; K2=( ["pre$'x'post"]=7 )
printf 'k2=%s\n' "$((K2["pre$'x'post"]))"
declare -A K3; K3=( ["pre$'x'post"]=a ["pre$'x'post"]=b )
printf 'k3keys='; printf '%s' "${!K3[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
printf 'k3=[%s]\n' "${K3["pre$'x'post"]}"
k="pre\$'x'post"
declare -A K4; K4=( ["pre$'x'post"]=w )
printf 'k4=[%s]\n' "${K4[$k]}"
K4[$k]=NEW
printf 'k4count=%s\n' "${#K4[@]}"
declare -A K5; K5=( ["pre$'x'post"]=v )
for kk in "${!K5[@]}"; do printf 'k5=[%s]\n' "${K5[$kk]}"; done
declare -A K6; K6=( ['a$'b'c']=z )
printf 'k6keys='; printf '%s' "${!K6[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
printf 'k6=[%s]\n' "${K6['a$'b'c']}"
declare -A K7; K7=( [pre$'x'post]=y )
printf 'k7keys='; printf '%s' "${!K7[@]}" | od -An -tx1 | tr -d ' \n'; printf '\n'
