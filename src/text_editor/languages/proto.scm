(comment) @comment
[(string) "\"proto2\"" "\"proto3\""] @string
(escape_sequence) @string.escape
[(int_lit) (float_lit)] @number
[(true) (false)] @boolean
[(key_type) (type) (message_name) (enum_name) (service_name)] @type
(rpc_name) @function
["syntax" "edition" "package" "option" "import" "service" "rpc" "returns"
 "message" "enum" "oneof" "repeated" "reserved" "to" "optional" "required"
 "public" "weak" "extend" "extensions" "stream" "map"] @keyword
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
