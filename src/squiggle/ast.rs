//! Source-spanned syntax tree for Squiggle programs.

/// A half-open byte range in Squiggle source text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span {
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset immediately after the final character.
    pub end: usize,
}

/// A parsed Squiggle module.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    /// Imports declared before executable statements.
    pub imports: Vec<Import>,
    /// Top-level bindings evaluated in source order.
    pub statements: Vec<Statement>,
    /// Optional final value produced by the module.
    pub result: Option<Expression>,
}

/// A named module import.
#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    /// Import path as authored.
    pub path: String,
    /// Local namespace receiving the imported module.
    pub name: String,
    /// Source range of the declaration.
    pub span: Span,
}

/// A binding decorator such as `@exportData`.
#[derive(Clone, Debug, PartialEq)]
pub struct Decorator {
    /// Decorator function name.
    pub name: String,
    /// Arguments passed to the decorator.
    pub arguments: Vec<Expression>,
    /// Source range of the decorator.
    pub span: Span,
}

/// A lexical binding or named function definition.
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    /// Decorators applied from nearest to furthest from the binding.
    pub decorators: Vec<Decorator>,
    /// Whether the binding is exported by its module.
    pub exported: bool,
    /// Bound identifier.
    pub name: String,
    /// Optional declared unit type.
    pub unit: Option<UnitType>,
    /// Expression evaluated and bound to `name`.
    pub value: Expression,
    /// Source range of the complete statement.
    pub span: Span,
}

/// A function parameter and its optional validation expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    /// Parameter identifier.
    pub name: String,
    /// Expression used as a runtime argument predicate.
    pub annotation: Option<Expression>,
    /// Optional declared unit type.
    pub unit: Option<UnitType>,
    /// Source range of the parameter.
    pub span: Span,
}

/// A source-spanned expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    /// Expression payload.
    pub kind: ExpressionKind,
    /// Source range of the expression.
    pub span: Span,
}

/// The expression forms supported by core Squiggle.
#[derive(Clone, Debug, PartialEq)]
pub enum ExpressionKind {
    /// A floating-point literal with an optional suffix unit.
    Number(f64, Option<String>),
    /// A Boolean literal.
    Boolean(bool),
    /// A UTF-8 string literal.
    String(String),
    /// A lexical or builtin identifier.
    Variable(String),
    /// An ordered collection.
    Array(Vec<Expression>),
    /// An insertion-ordered dictionary expression.
    Dictionary(Vec<(Expression, Expression)>),
    /// A lexical function.
    Lambda {
        /// Declared parameters.
        parameters: Vec<Parameter>,
        /// Function body.
        body: Box<Expression>,
        /// Optional declared return unit type.
        return_unit: Option<UnitType>,
    },
    /// A scoped sequence of bindings followed by a result.
    Block {
        /// Local bindings.
        statements: Vec<Statement>,
        /// Block result.
        result: Box<Expression>,
    },
    /// A prefix operation.
    Unary {
        /// Operator spelling.
        operator: String,
        /// Operand.
        expression: Box<Expression>,
    },
    /// An infix operation.
    Binary {
        /// Operator spelling.
        operator: String,
        /// Left operand.
        left: Box<Expression>,
        /// Right operand.
        right: Box<Expression>,
    },
    /// A lazy conditional expression.
    Conditional {
        /// Condition evaluated first.
        condition: Box<Expression>,
        /// Branch evaluated when true.
        when_true: Box<Expression>,
        /// Branch evaluated when false.
        when_false: Box<Expression>,
    },
    /// A function invocation.
    Call {
        /// Callable expression.
        function: Box<Expression>,
        /// Positional arguments.
        arguments: Vec<Expression>,
    },
    /// A dictionary field or array index lookup.
    Lookup {
        /// Value being indexed.
        value: Box<Expression>,
        /// Field name or numeric index.
        key: Box<Expression>,
    },
    /// A pipeline that prepends its left value to a call.
    Pipe {
        /// Value passed as the first argument.
        value: Box<Expression>,
        /// Callable expression.
        function: Box<Expression>,
        /// Remaining arguments.
        arguments: Vec<Expression>,
    },
}

/// A multiplicative unit type expression used by `::` signatures.
#[derive(Clone, Debug, PartialEq)]
pub enum UnitType {
    /// A named or numeric unit raised to a power.
    Factor {
        /// Unit name or numeric scale.
        name: String,
        /// Exponent applied to the factor.
        exponent: f64,
    },
    /// Product of two unit expressions.
    Product(Box<UnitType>, Box<UnitType>),
    /// Ratio of two unit expressions.
    Ratio(Box<UnitType>, Box<UnitType>),
}
