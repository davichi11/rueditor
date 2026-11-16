
; Component tags and intrinsic tags share the same visual role.
(jsx_opening_element name: (identifier) @tag)
(jsx_closing_element name: (identifier) @tag)
(jsx_self_closing_element name: (identifier) @tag)
(jsx_opening_element name: (member_expression) @tag)
(jsx_closing_element name: (member_expression) @tag)
(jsx_self_closing_element name: (member_expression) @tag)
(jsx_attribute (property_identifier) @attribute)
["<" ">" "</" "/>"] @punctuation.bracket
