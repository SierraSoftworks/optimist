//! Why a model could not be solved.

/// Why a model could not be solved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationError {
    /// A component adopts a type the catalogue does not define.
    UnknownType {
        /// The component.
        component: String,
        /// The type it named.
        component_type: String,
    },
    /// A relationship attaches a behaviour the catalogue does not define.
    UnknownMutator {
        /// The relationship, as source and destination.
        relationship: String,
        /// The behaviour it named.
        mutator: String,
    },
    /// A relationship attaches to a port the component's type does not declare.
    UnknownPort {
        /// The component.
        component: String,
        /// The port it named.
        port: String,
    },
    /// A relationship names no port on a type that declares several.
    AmbiguousPort {
        /// The component.
        component: String,
        /// Which side of the component was ambiguous.
        side: String,
    },
    /// Several relationships attach to a port that admits only one.
    CrowdedPort {
        /// The component.
        component: String,
        /// The port that was oversubscribed.
        port: String,
    },
    /// Nothing attaches to a port whose type declares it required.
    UnconnectedPort {
        /// The component.
        component: String,
        /// The port left unattached.
        port: String,
    },
    /// A scale unit refers to a component or unit the model does not declare.
    UnknownScaleUnit {
        /// The scale unit.
        scale_unit: String,
        /// The name it referred to.
        referenced: String,
    },
    /// A component is claimed directly by more than one scale unit.
    SharedMembership {
        /// The contested component.
        component: String,
    },
    /// Scale units enclose each other in a cycle.
    ScaleUnitCycle {
        /// A scale unit on the cycle.
        scale_unit: String,
    },
    /// An intervention rebinds a quantity the scratchpad does not declare.
    UnknownQuantity {
        /// The name it tried to rebind.
        quantity: String,
    },
    /// A model does not declare the requested intervention.
    UnknownIntervention {
        /// The identifier requested.
        intervention: String,
    },
    /// A required property was not supplied and has no default.
    MissingProperty {
        /// The component.
        component: String,
        /// The property.
        property: String,
    },
    /// A supplied property is not declared by the component's type.
    UnknownProperty {
        /// The component.
        component: String,
        /// The property.
        property: String,
    },
    /// Channels within one component refer to each other in a cycle.
    ChannelCycle {
        /// The component.
        component: String,
        /// The channels that could not be ordered.
        channels: Vec<String>,
    },
    /// An expression could not be parsed.
    Syntax {
        /// Where the expression was declared.
        location: String,
        /// The first parser diagnostic.
        message: String,
    },
    /// An expression could not be evaluated.
    Evaluation {
        /// Where the expression was declared.
        location: String,
        /// The runtime diagnostic.
        message: String,
    },
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType {
                component,
                component_type,
            } => write!(
                formatter,
                "component '{component}' adopts unknown type '{component_type}'"
            ),
            Self::UnknownMutator {
                relationship,
                mutator,
            } => write!(
                formatter,
                "relationship {relationship} attaches unknown behaviour '{mutator}'"
            ),
            Self::UnknownPort { component, port } => {
                write!(formatter, "component '{component}' has no port '{port}'")
            }
            Self::AmbiguousPort { component, side } => write!(
                formatter,
                "component '{component}' declares several {side} ports, so a relationship must name which one it uses"
            ),
            Self::CrowdedPort { component, port } => write!(
                formatter,
                "port '{port}' of component '{component}' admits one relationship, and its channels read that one peer's figures rather than a reduction over several"
            ),
            Self::UnconnectedPort { component, port } => write!(
                formatter,
                "port '{port}' of component '{component}' has nothing attached, and its type divides work across it: an empty port answers everything instantly and without fail, so the design would report a peer it does not have as carrying its share perfectly"
            ),
            Self::UnknownScaleUnit {
                scale_unit,
                referenced,
            } => write!(
                formatter,
                "scale unit '{scale_unit}' refers to '{referenced}', which the model does not declare"
            ),
            Self::SharedMembership { component } => write!(
                formatter,
                "component '{component}' belongs to more than one scale unit; nest the units instead"
            ),
            Self::ScaleUnitCycle { scale_unit } => {
                write!(formatter, "scale unit '{scale_unit}' encloses itself")
            }
            Self::UnknownQuantity { quantity } => write!(
                formatter,
                "'{quantity}' is not a scratchpad quantity, so rebinding it would change nothing"
            ),
            Self::UnknownIntervention { intervention } => {
                write!(
                    formatter,
                    "the model declares no intervention '{intervention}'"
                )
            }
            Self::MissingProperty {
                component,
                property,
            } => write!(
                formatter,
                "component '{component}' does not supply required property '{property}'"
            ),
            Self::UnknownProperty {
                component,
                property,
            } => write!(
                formatter,
                "component '{component}' supplies '{property}', which its type does not declare"
            ),
            Self::ChannelCycle {
                component,
                channels,
            } => write!(
                formatter,
                "channels {channels:?} of component '{component}' refer to each other in a cycle"
            ),
            Self::Syntax { location, message } => {
                write!(formatter, "{location} does not parse: {message}")
            }
            Self::Evaluation { location, message } => {
                write!(formatter, "{location} failed to evaluate: {message}")
            }
        }
    }
}

impl std::error::Error for EvaluationError {}
