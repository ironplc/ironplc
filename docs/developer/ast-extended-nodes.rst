Extended AST Node Documentation
===============================

This document provides detailed documentation for all AST (Abstract Syntax Tree) node types introduced by IronPLC's extended syntax features. Each node type includes its structure, usage patterns, semantic meaning, and code generation implications.

Node Type Conventions
---------------------

All extended AST nodes follow these conventions:

- **Derive Traits**: All nodes derive ``Clone``, ``Debug``, ``PartialEq``, and ``Recurse``
- **Source Spans**: All nodes include a ``span: SourceSpan`` field for error reporting
- **Naming**: Node types use descriptive names ending in ``Declaration``, ``Expression``, or ``Statement``
- **Documentation**: Each node includes comprehensive rustdoc comments with examples

Common Patterns
~~~~~~~~~~~~~~~

.. code-block:: rust

    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ExampleNode {
        pub field: Type,
        pub span: SourceSpan,
    }

External Function Nodes
-----------------------

ExternalFunctionDeclaration
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Represents function declarations with external linkage annotations.

.. code-block:: rust

    /// External function declaration with annotation support.
    /// 
    /// External functions are declared but not implemented in the current
    /// compilation unit. They are linked from external libraries or runtime systems.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// {external}
    /// FUNCTION MathSin : REAL
    ///     VAR_INPUT
    ///         angle : REAL;
    ///     END_VAR
    /// END_FUNCTION
    /// 
    /// @EXTERNAL
    /// FUNCTION SystemCall : BOOL
    ///     VAR_INPUT
    ///         command : STRING;
    ///     END_VAR
    /// END_FUNCTION
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ExternalFunctionDeclaration {
        /// Function name identifier
        pub name: Id,
        
        /// Return type of the function
        pub return_type: TypeName,
        
        /// Function parameters (may include reference parameters)
        pub parameters: Vec<VarDecl>,
        
        /// Type of external annotation used
        pub annotation: ExternalAnnotation,
        
        /// Source location for error reporting
        pub span: SourceSpan,
    }

**Semantic Analysis:**

- Added to symbol table without implementation body
- Parameters are type-checked for validity
- Reference parameters are validated for lvalue requirements at call sites

**Code Generation:**

- Generates external linkage declarations in target language
- Reference parameters are compiled to pointer types
- No function body is generated

ExternalAnnotation
~~~~~~~~~~~~~~~~~~

Enumeration of supported external function annotation types.

.. code-block:: rust

    /// External function annotation types.
    /// 
    /// IronPLC supports two annotation syntaxes for external functions:
    /// - Curly brace syntax: `{external}`
    /// - At-symbol syntax: `@EXTERNAL`
    /// 
    /// Both annotations have identical semantic meaning.
    #[derive(Clone, Debug, PartialEq)]
    pub enum ExternalAnnotation {
        /// Curly brace annotation: `{external}`
        CurlyBrace,
        
        /// At-symbol annotation: `@EXTERNAL`
        AtSymbol,
    }

Reference Parameter Nodes
-------------------------

ReferenceParameterDeclaration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Enhanced variable declaration supporting reference parameter annotation.

.. code-block:: rust

    /// Variable declaration with reference parameter support.
    /// 
    /// Reference parameters are annotated with `{ref}` and enable
    /// pass-by-reference semantics for function parameters.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// FUNCTION SwapValues
    ///     VAR_INPUT
    ///         {ref} a : INT;
    ///         {ref} b : INT;
    ///     END_VAR
    /// END_FUNCTION
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ReferenceParameterDeclaration {
        /// Parameter name
        pub name: Id,
        
        /// Parameter type
        pub parameter_type: TypeName,
        
        /// Whether this parameter is passed by reference
        pub is_reference: bool,
        
        /// Optional default value
        pub default_value: Option<Constant>,
        
        /// Source location
        pub span: SourceSpan,
    }

**Usage in VarDecl:**
Reference parameters are represented using the existing ``VarDecl`` structure with ``DeclarationQualifier::Reference``.

Class and Method Nodes
----------------------

ClassDeclaration
~~~~~~~~~~~~~~~~

Represents object-oriented class declarations with variables and methods.

.. code-block:: rust

    /// Class declaration supporting object-oriented programming.
    /// 
    /// Classes encapsulate data (variables) and behavior (methods) into
    /// reusable types. Class instances can be created and methods can be
    /// called on those instances.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// CLASS Motor
    ///     VAR
    ///         speed : REAL;
    ///         running : BOOL;
    ///     END_VAR
    ///     
    ///     METHOD Start : BOOL
    ///         running := TRUE;
    ///         Start := TRUE;
    ///     END_METHOD
    /// END_CLASS
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ClassDeclaration {
        /// Class name (becomes a type name)
        pub name: TypeName,
        
        /// Class instance variables
        pub variables: Vec<VarDecl>,
        
        /// Class methods
        pub methods: Vec<MethodDeclaration>,
        
        /// Optional inheritance (future extension)
        pub base_class: Option<TypeName>,
        
        /// Source location
        pub span: SourceSpan,
    }

**Type System Integration:**

- Creates new type in type system
- Instance variables define memory layout
- Methods are added to method resolution table

MethodDeclaration
~~~~~~~~~~~~~~~~~

Represents method declarations within classes.

.. code-block:: rust

    /// Method declaration within a class.
    /// 
    /// Methods have implicit access to class instance variables through
    /// an implicit `this` parameter. Methods can have return values and
    /// local variables.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// METHOD SetSpeed : BOOL
    ///     VAR_INPUT
    ///         target_speed : REAL;
    ///     END_VAR
    ///     VAR
    ///         old_speed : REAL;
    ///     END_VAR
    ///     
    ///     old_speed := speed;
    ///     speed := target_speed;
    ///     SetSpeed := TRUE;
    /// END_METHOD
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct MethodDeclaration {
        /// Method name
        pub name: Id,
        
        /// Optional return type (None for procedures)
        pub return_type: Option<TypeName>,
        
        /// Method parameters
        pub parameters: Vec<VarDecl>,
        
        /// Local variables
        pub local_variables: Vec<VarDecl>,
        
        /// Method body statements
        pub body: Vec<Statement>,
        
        /// Access modifier (future extension)
        pub access: AccessModifier,
        
        /// Source location
        pub span: SourceSpan,
    }

    /// Method access modifiers (future extension)
    #[derive(Clone, Debug, PartialEq)]
    pub enum AccessModifier {
        Public,
        Private,
        Protected,
    }

**Semantic Analysis:**

- Methods have access to class instance variables
- ``this`` context is implicitly available
- Method calls are resolved through class type information

Action Block Nodes
------------------

ActionBlockDeclaration
~~~~~~~~~~~~~~~~~~~~~~

Container for action declarations within programs.

.. code-block:: rust

    /// Action block declaration containing multiple actions.
    /// 
    /// Action blocks organize code into named, reusable sections within
    /// programs. Actions have access to program variables and can call
    /// each other.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// ACTIONS
    ///     ACTION Initialize
    ///         counter := 0;
    ///         state := 1;
    ///     END_ACTION
    ///     
    ///     ACTION Process
    ///         counter := counter + 1;
    ///     END_ACTION
    /// END_ACTIONS
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ActionBlockDeclaration {
        /// List of actions in this block
        pub actions: Vec<ActionDeclaration>,
        
        /// Source location
        pub span: SourceSpan,
    }

ActionDeclaration
~~~~~~~~~~~~~~~~~

Individual action declaration within an action block.

.. code-block:: rust

    /// Individual action declaration.
    /// 
    /// Actions are named code blocks that can be called from the main
    /// program body or from other actions. They have access to program
    /// variables and can declare local variables.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// ACTION ProcessData
    ///     VAR
    ///         temp_value : INT;
    ///     END_VAR
    ///     
    ///     temp_value := sensor_input;
    ///     IF temp_value > threshold THEN
    ///         alarm := TRUE;
    ///     END_IF;
    /// END_ACTION
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ActionDeclaration {
        /// Action name
        pub name: Id,
        
        /// Local variables (in addition to program variables)
        pub variables: Vec<VarDecl>,
        
        /// Action body statements
        pub body: Vec<Statement>,
        
        /// Source location
        pub span: SourceSpan,
    }

**Scope Rules:**

- Actions have access to all program-level variables
- Local variables are scoped to the action body
- Actions can call other actions in the same program

Reference Type Nodes
--------------------

ReferenceDeclaration
~~~~~~~~~~~~~~~~~~~~

Type declaration for reference types (REF_TO).

.. code-block:: rust

    /// Reference type declaration.
    /// 
    /// Reference types provide pointer-like functionality for indirect
    /// access to variables. They can be assigned addresses of variables
    /// and dereferenced to access the pointed-to values.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// TYPE
    ///     IntRef : REF_TO INT;
    ///     RealPtr : REF_TO REAL;
    /// END_TYPE
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct ReferenceDeclaration {
        /// Name of the reference type
        pub type_name: TypeName,
        
        /// Type being referenced
        pub referenced_type: TypeName,
        
        /// Whether null values are allowed (always true currently)
        pub nullable: bool,
        
        /// Source location
        pub span: SourceSpan,
    }

**Type System:**

- Creates new reference type in type system
- Supports null pointer values
- Enables address-of and dereference operations

ReferenceExpression
~~~~~~~~~~~~~~~~~~~

Expression nodes for reference operations.

.. code-block:: rust

    /// Reference operation expressions.
    /// 
    /// Represents address-of, dereference, and null literal expressions
    /// used with reference types.
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub enum ReferenceExpression {
        /// Address-of operation: `&variable`
        AddressOf {
            /// Target expression (must be lvalue)
            target: Box<Expression>,
            
            /// Source location
            span: SourceSpan,
        },
        
        /// Dereference operation: `pointer^` or `pointer^^`
        Dereference {
            /// Target expression (must be reference type)
            target: Box<Expression>,
            
            /// Number of dereference levels (^ operators)
            levels: usize,
            
            /// Source location
            span: SourceSpan,
        },
        
        /// Null literal: `NULL`
        NullLiteral {
            /// Source location
            span: SourceSpan,
        },
    }

**Semantic Rules:**

- Address-of requires lvalue expressions
- Dereference requires reference type expressions
- Multiple dereference levels are supported (e.g., ``ptr^^``)

Range Constraint Nodes
----------------------

RangeConstrainedType
~~~~~~~~~~~~~~~~~~~~

Type declaration with value range constraints.

.. code-block:: rust

    /// Range-constrained type declaration.
    /// 
    /// Defines types with specific value bounds for safety and optimization.
    /// Runtime checks can be generated to validate constraint violations.
    /// 
    /// # Examples
    /// 
    /// ```st
    /// TYPE
    ///     Percentage : DINT(0..100);
    ///     Temperature : REAL(-40.0..150.0);
    /// END_TYPE
    /// ```
    #[derive(Clone, Debug, PartialEq, Recurse)]
    pub struct RangeConstrainedType {
        /// Name of the constrained type
        pub type_name: TypeName,
        
        /// Base type being constrained
        pub base_type: TypeName,
        
        /// Minimum allowed value
        pub min_value: Option<Constant>,
        
        /// Maximum allowed value
        pub max_value: Option<Constant>,
        
        /// Default value within range
        pub default_value: Option<Constant>,
        
        /// Whether to generate runtime checks
        pub runtime_checks: bool,
        
        /// Source location
        pub span: SourceSpan,
    }

**Constraint Validation:**

- Compile-time validation where possible
- Runtime checks for dynamic values
- Constraint propagation through expressions

Enhanced Expression and Statement Types
---------------------------------------

The extended syntax also introduces several new expression and statement types for method calls, array access with bounds checking, struct member access, action calls, and continue statements. These follow the same patterns as the core AST nodes but provide enhanced functionality for the extended language features.

For complete implementation details, see the source code in ``ironplc/compiler/dsl/src/common.rs`` and related modules.

Node Traversal and Transformation
---------------------------------

All AST nodes support the visitor pattern through the ``Recurse`` trait, enabling systematic traversal and transformation of the AST for analysis and code generation phases.

Memory Layout and Serialization
-------------------------------

AST nodes are designed for efficient memory usage with boxed expressions for large trees, vector storage for collections, and optional serialization support for caching and debugging purposes.