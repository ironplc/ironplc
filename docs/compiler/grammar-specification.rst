====================
Grammar Specification
====================

This document describes the formal grammar for IronPLC's extended Structured Text syntax. The grammar is presented in Extended Backus-Naur Form (EBNF) notation and includes both standard IEC 61131-3 constructs and IronPLC's extensions.

Notation
========

The following notation is used in this grammar specification:

- ``::=`` defines a production rule
- ``|`` indicates alternatives
- ``[]`` indicates optional elements
- ``{}`` indicates zero or more repetitions
- ``()`` groups elements
- ``"text"`` indicates literal text
- ``<name>`` indicates a non-terminal symbol

Lexical Elements
================

Tokens
------

.. code-block:: ebnf

   identifier ::= letter { letter | digit | "_" }
   integer ::= digit { digit }
   real ::= digit { digit } "." digit { digit } [ exponent ]
   exponent ::= ( "E" | "e" ) [ "+" | "-" ] digit { digit }
   string ::= "'" { character } "'"
   
   letter ::= "A" | "B" | ... | "Z" | "a" | "b" | ... | "z"
   digit ::= "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"

Keywords
--------

.. code-block:: ebnf

   keyword ::= "PROGRAM" | "END_PROGRAM" | "FUNCTION" | "END_FUNCTION" |
               "FUNCTION_BLOCK" | "END_FUNCTION_BLOCK" | "CLASS" | "END_CLASS" |
               "METHOD" | "END_METHOD" | "ACTION" | "END_ACTION" | "ACTIONS" |
               "END_ACTIONS" | "VAR" | "END_VAR" | "VAR_INPUT" | "VAR_OUTPUT" |
               "VAR_IN_OUT" | "VAR_TEMP" | "VAR_GLOBAL" | "VAR_ACCESS" |
               "TYPE" | "END_TYPE" | "STRUCT" | "END_STRUCT" | "ARRAY" | "OF" |
               "REF_TO" | "IF" | "THEN" | "ELSE" | "ELSIF" | "END_IF" |
               "CASE" | "OF" | "END_CASE" | "FOR" | "TO" | "BY" | "DO" |
               "END_FOR" | "WHILE" | "END_WHILE" | "REPEAT" | "UNTIL" |
               "END_REPEAT" | "CONTINUE" | "EXIT" | "RETURN" | "NULL" |
               "TRUE" | "FALSE" | "AND" | "OR" | "XOR" | "NOT" | "MOD"

Operators
---------

.. code-block:: ebnf

   operator ::= "+" | "-" | "*" | "/" | "=" | "<>" | "<" | ">" | "<=" | ">=" |
                ":=" | "&" | "^" | "." | "[" | "]" | "(" | ")" | "," | ";" |
                ".." | "**"

Comments
--------

.. code-block:: ebnf

   iec_comment ::= "(*" { character } "*)"
   c_comment ::= "//" { character } newline

Annotations
-----------

.. code-block:: ebnf

   external_annotation ::= "{external}" | "@EXTERNAL"
   ref_annotation ::= "{ref}"

Program Structure
=================

Library Element
---------------

.. code-block:: ebnf

   library_element ::= data_type_declaration |
                       function_declaration |
                       function_block_declaration |
                       class_declaration |
                       program_declaration |
                       configuration_declaration

Data Type Declarations
======================

.. code-block:: ebnf

   data_type_declaration ::= "TYPE" type_declaration { type_declaration } "END_TYPE"
   
   type_declaration ::= simple_type_declaration |
                        subrange_type_declaration |
                        enumerated_type_declaration |
                        array_type_declaration |
                        structure_type_declaration |
                        reference_type_declaration |
                        range_constrained_type_declaration
   
   simple_type_declaration ::= identifier ":" simple_specification ";"
   
   subrange_type_declaration ::= identifier ":" subrange_specification ";"
   
   enumerated_type_declaration ::= identifier ":" "(" identifier_list ")" ";"
   
   array_type_declaration ::= identifier ":" "ARRAY" "[" subrange_list "]" "OF" data_type_access ";"
   
   structure_type_declaration ::= identifier ":" "STRUCT" structure_element_declaration_list "END_STRUCT" ";"
   
   reference_type_declaration ::= identifier ":" "REF_TO" data_type_access ";"
   
   range_constrained_type_declaration ::= identifier ":" elementary_type_name "(" constant ".." constant ")" ";"

Variable Declarations
=====================

.. code-block:: ebnf

   var_declaration ::= "VAR" [ "CONSTANT" ] variable_list "END_VAR" |
                       "VAR_INPUT" [ "RETAIN" | "NON_RETAIN" ] input_declarations "END_VAR" |
                       "VAR_OUTPUT" [ "RETAIN" | "NON_RETAIN" ] output_declarations "END_VAR" |
                       "VAR_IN_OUT" var_declaration_list "END_VAR" |
                       "VAR_TEMP" temp_var_declaration_list "END_VAR" |
                       "VAR_GLOBAL" [ "CONSTANT" ] [ "RETAIN" | "NON_RETAIN" ] global_var_declarations "END_VAR" |
                       "VAR_ACCESS" access_declarations "END_VAR"
   
   variable_list ::= variable_declaration { variable_declaration }
   
   variable_declaration ::= identifier_list ":" [ array_specification ] simple_specification [ ":=" constant ] ";"
   
   input_declarations ::= input_declaration { input_declaration }
   
   input_declaration ::= [ ref_annotation ] identifier_list ":" [ "RETAIN" | "NON_RETAIN" ] 
                         [ array_specification ] simple_specification [ ":=" constant ] ";"

Function Declarations
=====================

.. code-block:: ebnf

   function_declaration ::= [ external_annotation ] "FUNCTION" identifier ":" elementary_type_name
                            [ var_declaration_list ]
                            [ function_body ]
                            "END_FUNCTION"
   
   function_body ::= statement_list

Function Block Declarations
===========================

.. code-block:: ebnf

   function_block_declaration ::= "FUNCTION_BLOCK" identifier
                                  [ var_declaration_list ]
                                  [ function_block_body ]
                                  "END_FUNCTION_BLOCK"
   
   function_block_body ::= statement_list

Class Declarations (Extension)
==============================

.. code-block:: ebnf

   class_declaration ::= "CLASS" identifier
                         [ var_declaration_list ]
                         [ method_declaration_list ]
                         "END_CLASS"
   
   method_declaration_list ::= method_declaration { method_declaration }
   
   method_declaration ::= "METHOD" identifier [ ":" elementary_type_name ]
                          [ var_declaration_list ]
                          [ method_body ]
                          "END_METHOD"
   
   method_body ::= statement_list

Program Declarations
====================

.. code-block:: ebnf

   program_declaration ::= "PROGRAM" identifier
                           [ var_declaration_list ]
                           [ program_body ]
                           [ action_block_declaration ]
                           "END_PROGRAM"
   
   program_body ::= statement_list

Action Block Declarations (Extension)
=====================================

.. code-block:: ebnf

   action_block_declaration ::= "ACTIONS" action_declaration_list "END_ACTIONS"
   
   action_declaration_list ::= action_declaration { action_declaration }
   
   action_declaration ::= "ACTION" identifier
                          [ var_declaration_list ]
                          [ action_body ]
                          "END_ACTION"
   
   action_body ::= statement_list

Statements
==========

.. code-block:: ebnf

   statement_list ::= statement { statement }
   
   statement ::= assignment_statement |
                 subprogram_control_statement |
                 selection_statement |
                 iteration_statement |
                 action_call_statement |
                 continue_statement |
                 exit_statement |
                 return_statement
   
   assignment_statement ::= variable ":=" expression ";"
   
   subprogram_control_statement ::= function_call ";" |
                                    method_call ";"
   
   selection_statement ::= if_statement | case_statement
   
   iteration_statement ::= for_statement | while_statement | repeat_statement
   
   action_call_statement ::= identifier "(" ")" ";"
   
   continue_statement ::= "CONTINUE" ";"
   
   exit_statement ::= "EXIT" ";"
   
   return_statement ::= "RETURN" ";"

Control Flow Statements
=======================

.. code-block:: ebnf

   if_statement ::= "IF" boolean_expression "THEN" statement_list
                    { "ELSIF" boolean_expression "THEN" statement_list }
                    [ "ELSE" statement_list ]
                    "END_IF"
   
   case_statement ::= "CASE" expression "OF" case_element_list
                      [ "ELSE" statement_list ]
                      "END_CASE"
   
   case_element_list ::= case_element { case_element }
   
   case_element ::= case_list ":" statement_list
   
   case_list ::= case_list_element { "," case_list_element }
   
   case_list_element ::= subrange | constant
   
   for_statement ::= "FOR" identifier ":=" expression "TO" expression [ "BY" expression ]
                     "DO" statement_list "END_FOR"
   
   while_statement ::= "WHILE" boolean_expression "DO" statement_list "END_WHILE"
   
   repeat_statement ::= "REPEAT" statement_list "UNTIL" boolean_expression "END_REPEAT"

Expressions
===========

.. code-block:: ebnf

   expression ::= xor_expression { "OR" xor_expression }
   
   xor_expression ::= and_expression { "XOR" and_expression }
   
   and_expression ::= equality_expression { ( "AND" | "&" ) equality_expression }
   
   equality_expression ::= comparison_expression { ( "=" | "<>" ) comparison_expression }
   
   comparison_expression ::= add_expression { ( "<" | ">" | "<=" | ">=" ) add_expression }
   
   add_expression ::= term { ( "+" | "-" ) term }
   
   term ::= factor { ( "*" | "/" | "MOD" ) factor }
   
   factor ::= power_expression
   
   power_expression ::= unary_expression [ "**" unary_expression ]
   
   unary_expression ::= [ ( "+" | "-" | "NOT" ) ] primary_expression
   
   primary_expression ::= constant |
                          variable |
                          function_call |
                          method_call |
                          reference_expression |
                          "(" expression ")"

Reference Expressions (Extension)
=================================

.. code-block:: ebnf

   reference_expression ::= address_of_expression |
                            dereference_expression |
                            null_expression
   
   address_of_expression ::= "&" variable
   
   dereference_expression ::= variable "^" { "^" }
   
   null_expression ::= "NULL"

Variables and Access
====================

.. code-block:: ebnf

   variable ::= direct_variable |
                symbolic_variable
   
   symbolic_variable ::= variable_name { subscript_list | "." field_selector }
   
   subscript_list ::= "[" subscript { "," subscript } "]"
   
   subscript ::= expression
   
   field_selector ::= identifier
   
   variable_name ::= identifier

Function and Method Calls
=========================

.. code-block:: ebnf

   function_call ::= function_name "(" [ parameter_assignment_list ] ")"
   
   method_call ::= variable "." method_name "(" [ parameter_assignment_list ] ")"
   
   parameter_assignment_list ::= parameter_assignment { "," parameter_assignment }
   
   parameter_assignment ::= [ formal_parameter ":=" ] actual_parameter
   
   formal_parameter ::= identifier
   
   actual_parameter ::= expression

Constants
=========

.. code-block:: ebnf

   constant ::= numeric_literal |
                character_string |
                time_literal |
                bit_string_literal |
                boolean_literal |
                null_literal
   
   numeric_literal ::= integer_literal | real_literal
   
   integer_literal ::= [ integer_type_name "#" ] ( decimal_integer | binary_integer | octal_integer | hex_integer )
   
   real_literal ::= [ real_type_name "#" ] signed_real
   
   boolean_literal ::= ( "BOOL#" | "BOOLEAN#" ) ( "1" | "0" | "TRUE" | "FALSE" ) |
                       "TRUE" | "FALSE"
   
   null_literal ::= "NULL"

Configuration Declarations
==========================

.. code-block:: ebnf

   configuration_declaration ::= "CONFIGURATION" identifier
                                 [ global_var_declarations ]
                                 [ resource_declaration_list ]
                                 [ access_declarations ]
                                 "END_CONFIGURATION"
   
   resource_declaration ::= "RESOURCE" identifier "ON" processor_type
                            [ global_var_declarations ]
                            [ single_resource_declaration_list ]
                            "END_RESOURCE"

Extended Syntax Features Summary
================================

The following extensions to standard IEC 61131-3 are supported:

1. **External Function Annotations**
   - ``{external}`` and ``@EXTERNAL`` annotations for function declarations
   - Functions declared as external do not require implementation bodies

2. **Reference Parameter Annotations**
   - ``{ref}`` annotation for input parameters
   - Enables pass-by-reference semantics

3. **C-Style Comments**
   - ``//`` line comments in addition to ``(* *)`` block comments
   - Comments are ignored during parsing

4. **Class and Method Declarations**
   - Object-oriented programming constructs
   - Classes can contain variables and methods
   - Methods have access to class instance variables

5. **Action Block Declarations**
   - Named code blocks within programs
   - Actions can be called by name from the main program body

6. **Reference Types and Operations**
   - ``REF_TO`` type declarations for pointer-like functionality
   - Address-of operator ``&`` and dereference operator ``^``
   - ``NULL`` literal for null pointer values

7. **Continue Statement**
   - ``CONTINUE`` statement for loop control
   - Skips to next iteration of innermost containing loop

8. **Range-Constrained Types**
   - Type declarations with value bounds: ``TypeName(min..max)``
   - Runtime validation of constraint violations

9. **Enhanced Array and Struct Operations**
   - Multi-dimensional array indexing with bounds checking
   - Nested struct member access with dot notation

Precedence and Associativity
============================

Operator precedence (highest to lowest):

1. ``^`` (dereference) - left associative
2. ``**`` (exponentiation) - right associative  
3. ``+``, ``-`` (unary), ``NOT`` - right associative
4. ``*``, ``/``, ``MOD`` - left associative
5. ``+``, ``-`` (binary) - left associative
6. ``<``, ``>``, ``<=``, ``>=`` - left associative
7. ``=``, ``<>`` - left associative
8. ``&``, ``AND`` - left associative
9. ``XOR`` - left associative
10. ``OR`` - left associative

Error Productions
=================

The grammar includes error recovery productions for common syntax errors:

- Missing semicolons in statements
- Unmatched parentheses and brackets
- Invalid annotation placement
- Malformed reference operations
- Invalid continue statement placement

This grammar specification provides the complete formal definition of IronPLC's extended Structured Text syntax, enabling precise parsing and validation of programs using these enhanced features.