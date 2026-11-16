; CMake grammar keywords are named nodes, matched case-insensitively by the parser.
[(line_comment) (bracket_comment)] @comment
(escape_sequence) @string.escape
[(quoted_argument) (bracket_argument)] @string
(variable) @variable
[(if) (elseif) (else) (endif) (foreach) (endforeach) (while) (endwhile)
 (function) (endfunction) (macro) (endmacro) (block) (endblock)] @keyword
(normal_command (identifier) @function)
["(" ")" "{" "}"] @punctuation.bracket
["$" ";"] @punctuation.special
