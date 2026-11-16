(comment) @comment
"export" @keyword
(variable_assignment name: (variable_name) @property)
(variable_assignment value: (word) @string)
[(string) (raw_string)] @string
(simple_expansion (variable_name) @variable.special)
(expansion (variable_name) @variable.special)
["=" "$" "${" "}"] @operator
