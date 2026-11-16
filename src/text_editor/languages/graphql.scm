(comment) @comment
(string_value) @string
[(int_value) (float_value)] @number
(boolean_value) @boolean
(null_value) @constant
(variable) @variable
(named_type (name) @type)
(field (name) @property)
(field_definition (name) @property)
(argument (name) @attribute)
(directive (name) @attribute)
["query" "mutation" "subscription" "fragment" "on" "schema" "scalar"
 "type" "interface" "implements" "union" "enum" "input" "extend" "directive" "repeatable"] @keyword
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[":" "!" "=" "|" "&" "@" "..." "$"] @punctuation.delimiter
