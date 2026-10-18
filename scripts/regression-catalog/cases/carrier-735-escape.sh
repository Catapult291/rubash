# 735.1/735.2 + #729 interplay: user U+E4xx data rides the E400 literal
# escape and must round-trip verbatim; the #729 matcher normalization must
# never consume an E400 escape.
x=$'\uE401'
printf '%s' "$x" | od -An -tx1
echo "len=${#x}"
y=$'\uE301'
echo "len301=${#y}"
[[ $x == *$'\uE401'* ]]; echo "self-match=$?"
