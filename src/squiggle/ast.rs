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
        /// Operator applied to the operand.
        operator: UnaryOperator,
        /// Operand.
        expression: Box<Expression>,
    },
    /// An infix operation.
    Binary {
        /// Operator applied to the operands.
        operator: BinaryOperator,
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

/// A prefix operator, resolved from its spelling while the module is parsed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOperator {
    /// Logical negation, `!`.
    Not,
    /// Arithmetic negation, `-`.
    Negate,
    /// Arithmetic negation applied per draw, `.-`.
    NegateEach,
}

impl UnaryOperator {
    /// Resolves an operator from the way it was written, if it is one.
    pub fn parse(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "!" => Self::Not,
            "-" => Self::Negate,
            ".-" => Self::NegateEach,
            _ => return None,
        })
    }

    /// Returns the spelling this operator was written with.
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Not => "!",
            Self::Negate => "-",
            Self::NegateEach => ".-",
        }
    }
}

/// An infix operator, resolved from its spelling while the module is parsed.
///
/// Held as an operator rather than as the text that produced it because the
/// spelling is only useful in a diagnostic, while the choice it encodes is made
/// again for every evaluation of the expression. Deciding it once, where the
/// text is already in hand, leaves the evaluator with a value to switch on.
///
/// The `.`-prefixed forms are the elementwise spellings. They agree with their
/// plain counterparts on numbers and distributions and differ on the operands
/// `+` also accepts: `"a" + "b"` joins two strings, where `"a" .+ "b"` is an
/// arithmetic operation on values that are not numbers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOperator {
    /// Addition, `+`.
    Add,
    /// Subtraction, `-`.
    Subtract,
    /// Multiplication, `*`.
    Multiply,
    /// Division, `/`.
    Divide,
    /// Exponentiation, `^`.
    Power,
    /// Addition applied per draw, `.+`.
    AddEach,
    /// Subtraction applied per draw, `.-`.
    SubtractEach,
    /// Multiplication applied per draw, `.*`.
    MultiplyEach,
    /// Division applied per draw, `./`.
    DivideEach,
    /// Exponentiation applied per draw, `.^`.
    PowerEach,
    /// Equality, `==`.
    Equal,
    /// Inequality, `!=`.
    NotEqual,
    /// Ordering, `<`.
    Less,
    /// Ordering, `<=`.
    LessOrEqual,
    /// Ordering, `>`.
    Greater,
    /// Ordering, `>=`.
    GreaterOrEqual,
    /// Short-circuiting conjunction, `&&`.
    And,
    /// Short-circuiting disjunction, `||`.
    Or,
    /// A lognormal credible interval, `to`.
    Interval,
}

impl BinaryOperator {
    /// Resolves an operator from the way it was written, if it is one.
    pub fn parse(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "+" => Self::Add,
            "-" => Self::Subtract,
            "*" => Self::Multiply,
            "/" => Self::Divide,
            "^" => Self::Power,
            ".+" => Self::AddEach,
            ".-" => Self::SubtractEach,
            ".*" => Self::MultiplyEach,
            "./" => Self::DivideEach,
            ".^" => Self::PowerEach,
            "==" => Self::Equal,
            "!=" => Self::NotEqual,
            "<" => Self::Less,
            "<=" => Self::LessOrEqual,
            ">" => Self::Greater,
            ">=" => Self::GreaterOrEqual,
            "&&" => Self::And,
            "||" => Self::Or,
            "to" => Self::Interval,
            _ => return None,
        })
    }

    /// Returns the spelling this operator was written with.
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Power => "^",
            Self::AddEach => ".+",
            Self::SubtractEach => ".-",
            Self::MultiplyEach => ".*",
            Self::DivideEach => "./",
            Self::PowerEach => ".^",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
            Self::And => "&&",
            Self::Or => "||",
            Self::Interval => "to",
        }
    }
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
