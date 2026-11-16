; Kernel macros look like calls in the grammar; give them keyword precedence.
(call target: (identifier) @keyword
 (#any-of? @keyword "def" "defp" "defmodule" "defmacro" "defmacrop" "defprotocol"
 "defimpl" "defstruct" "defexception" "defdelegate" "defguard" "defguardp"
 "alias" "case" "cond" "for" "if" "import" "quote" "raise" "receive" "require"
 "reraise" "super" "throw" "try" "unless" "unquote" "unquote_splicing" "use" "with"))
