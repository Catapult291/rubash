# 735.1/735.2 + #729 interplay: user U+E4xx data rides the E400 literal
# escape and must round-trip verbatim; the #729 matcher normalization must
# never consume an E400 escape.
# The catalog runner pins the GNU oracle to LC_ALL=C, under which bash
# cannot encode PUA codepoints and falls back to literal `\uXXXX` text.
# Pin C.UTF-8 inside the case so both sides run the UTF-8 domain where
# they already agree byte-for-byte (for rubash C.UTF-8 selects the same
# UTF-8 model its unpinned arm uses anyway).
export LC_ALL=C.UTF-8
x=$'\uE401'
printf '%s' "$x" | od -An -tx1
echo "len=${#x}"
y=$'\uE301'
echo "len301=${#y}"
[[ $x == *$'\uE401'* ]]; echo "self-match=$?"
