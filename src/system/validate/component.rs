//! Checking a component type's declarations against each other.

use std::collections::BTreeSet;

use crate::system::{expression::RESERVED, manifest::ComponentType};

use super::{
    ComponentTypeError,
    checks::{
        validate_name, validate_publication, validate_references, validate_shaped_identifier,
        validate_syntax, validate_unit,
    },
};

impl ComponentType {
    /// Parses and validates a component type from its YAML manifest.
    ///
    /// ```
    /// use optimist::system::ComponentType;
    ///
    /// let manifest = "
    /// id: token-bucket
    /// name: Token bucket
    /// ports:
    ///   in:
    ///     requests:
    ///       arity: many
    ///       publishes:
    ///         success: admitted_ratio
    ///         latency: '0'
    ///   out:
    ///     downstream:
    ///       arity: one
    ///       publishes:
    ///         rate: admitted
    /// properties:
    ///   refill:
    ///     unit: op/s
    ///   burst:
    ///     unit: op
    ///     default: '0'
    /// channels:
    ///   arriving:
    ///     unit: op/s
    ///     expression: in.requests.rate
    ///   admitted:
    ///     unit: op/s
    ///     expression: min([arriving, refill])
    ///   admitted_ratio:
    ///     unit: '1'
    ///     expression: min([admitted / max([arriving, 0.000001]), 1])
    /// constraints:
    ///   throughput:
    ///     demand: arriving
    ///     limit: refill
    /// ";
    /// let component = ComponentType::parse(manifest)?;
    ///
    /// assert_eq!(component.id.as_str(), "token-bucket");
    /// // A property without a default is one an author must supply.
    /// assert!(component.properties["refill"].is_required());
    /// assert!(!component.properties["burst"].is_required());
    /// // Ports publish channels the component has already worked out, which is
    /// // how one component's result becomes another's input.
    /// assert_eq!(component.ports.outbound["downstream"].publishes["rate"], "admitted");
    /// assert_eq!(component.ports.inbound["requests"].publishes["success"], "admitted_ratio");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse(manifest: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let component: Self = serde_yaml_ng::from_str(manifest)?;
        component.validate()?;
        Ok(component)
    }

    /// Checks every invariant the evaluator relies on.
    pub fn validate(&self) -> Result<(), ComponentTypeError> {
        validate_shaped_identifier(self.id.as_str())?;
        let surface = self.surface()?;
        let visible = surface
            .iter()
            .cloned()
            .chain(RESERVED.iter().map(|name| (*name).to_owned()))
            .collect::<BTreeSet<_>>();
        for (name, channel) in &self.channels {
            validate_references(&format!("channel '{name}'"), &channel.expression, &visible)?;
        }
        for (name, constraint) in &self.constraints {
            validate_references(
                &format!("constraint '{name}' demand"),
                &constraint.demand,
                &visible,
            )?;
            validate_references(
                &format!("constraint '{name}' limit"),
                &constraint.limit,
                &visible,
            )?;
        }
        self.validate_ports(&surface)
    }

    /// Names this type declares, checked for shape and for collisions.
    fn surface(&self) -> Result<BTreeSet<String>, ComponentTypeError> {
        let mut surface = BTreeSet::new();
        for (name, property) in &self.properties {
            validate_name("property", name)?;
            validate_unit(&format!("property '{name}'"), &property.unit)?;
            if let Some(default) = &property.default {
                validate_syntax(&format!("property '{name}' default"), default)?;
            }
            surface.insert(name.clone());
        }
        for (name, channel) in &self.channels {
            validate_name("channel", name)?;
            validate_unit(&format!("channel '{name}'"), &channel.unit)?;
            if !surface.insert(name.clone()) {
                return Err(ComponentTypeError::Duplicate {
                    value: name.clone(),
                });
            }
        }
        Ok(surface)
    }

    /// A port publishes quantities the component has already worked out, so its
    /// expressions see the properties and channels but not the reserved flow
    /// bindings: a port cannot read the wire it is publishing onto.
    fn validate_ports(&self, surface: &BTreeSet<String>) -> Result<(), ComponentTypeError> {
        let ports = self
            .ports
            .inbound
            .iter()
            .map(|(name, port)| (format!("inbound port '{name}'"), port, true))
            .chain(
                self.ports
                    .outbound
                    .iter()
                    .map(|(name, port)| (format!("outbound port '{name}'"), port, false)),
            )
            .collect::<Vec<_>>();
        for (location, port, _) in &ports {
            for (signal, source) in &port.publishes {
                validate_name("published signal", signal)?;
                validate_references(&format!("{location} signal '{signal}'"), source, surface)?;
            }
        }
        // Checked after every expression, so a type with a typo in it is told
        // about the typo rather than about the signal the typo left unpublished.
        for (location, port, inbound) in &ports {
            validate_publication(location, port, *inbound)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(body: &str) -> String {
        format!("id: probe\nname: Probe\n{body}")
    }

    #[test]
    fn a_minimal_type_validates() {
        let component = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\nchannels:\n  served:\n    unit: op/s\n    expression: min([in.requests.rate, limit])\n",
        ))
        .expect("valid");
        assert_eq!(component.channels.len(), 1);
    }

    #[test]
    fn a_property_without_a_default_is_required() {
        let component = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\n  spare:\n    unit: op/s\n    default: '0'\n",
        ))
        .expect("valid");
        assert!(component.properties["limit"].is_required());
        assert!(!component.properties["spare"].is_required());
    }

    #[test]
    fn a_mistyped_reference_is_caught_at_load_time() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\nchannels:\n  served:\n    unit: op/s\n    expression: min([in.requests.rate, limitt])\n",
        ))
        .expect_err("unresolved");
        assert!(error.to_string().contains("limitt"), "{error}");
    }

    #[test]
    fn reserved_bindings_resolve() {
        ComponentType::parse(&manifest(
            "properties:\n  drain:\n    unit: op/s\nchannels:\n  backlog:\n    unit: op\n    expression: max([prev.backlog + (in.requests.rate - drain) * dt, 0])\n  age:\n    unit: s\n    expression: t\n",
        ))
        .expect("valid");
    }

    #[test]
    fn a_channel_may_reference_another_channel() {
        ComponentType::parse(&manifest(
            "channels:\n  arrivals:\n    unit: op/s\n    expression: in.requests.rate\n  doubled:\n    unit: op/s\n    expression: arrivals * 2\n",
        ))
        .expect("valid");
    }

    #[test]
    fn a_name_declared_twice_is_rejected() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  rate:\n    unit: op/s\nchannels:\n  rate:\n    unit: op/s\n    expression: '1'\n",
        ))
        .expect_err("duplicate");
        assert!(
            error.to_string().contains("both a property and a channel"),
            "{error}"
        );
    }

    #[test]
    fn a_port_may_publish_a_property_or_a_channel() {
        ComponentType::parse(&manifest(
            "properties:\n  payload:\n    unit: B/op\nchannels:\n  served:\n    unit: op/s\n    expression: in.requests.rate\nports:\n  in:\n    requests:\n      publishes:\n        payload: payload\n        latency: '0'\n        success: '1'\n  out:\n    calls:\n      publishes:\n        rate: served\n",
        ))
        .expect("valid");
    }

    #[test]
    fn a_signal_travelling_one_way_cannot_be_published_the_other() {
        // A response leg carrying demand feeds a component's own load back into
        // its caller, and the loop that makes has no bound.
        let error = ComponentType::parse(&manifest(
            "channels:\n  served:\n    unit: op/s\n    expression: in.requests.rate\nports:\n  in:\n    requests:\n      publishes:\n        rate: served\n        latency: '0'\n        success: '1'\n",
        ))
        .expect_err("wrong way");
        assert!(error.to_string().contains("an outbound port"), "{error}");
    }

    #[test]
    fn the_engine_supplied_signals_cannot_be_published_at_all() {
        let error = ComponentType::parse(&manifest(
            "channels:\n  served:\n    unit: op/s\n    expression: in.requests.rate\nports:\n  out:\n    calls:\n      publishes:\n        rate: served\n        peers: served\n",
        ))
        .expect_err("engine supplied");
        assert!(error.to_string().contains("no port may state"), "{error}");
    }

    #[test]
    fn an_inbound_port_must_answer_with_a_latency_and_a_success() {
        // Omitting either is silent and flattering: the component reads as one
        // that answers instantly, or as one that cannot fail.
        let error = ComponentType::parse(&manifest(
            "ports:\n  in:\n    requests:\n      publishes:\n        success: '1'\n",
        ))
        .expect_err("no latency");
        assert!(error.to_string().contains("'latency'"), "{error}");
    }

    #[test]
    fn an_outbound_port_must_state_the_demand_it_places() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  payload:\n    unit: B/op\nports:\n  out:\n    calls:\n      publishes:\n        payload: payload\n",
        ))
        .expect_err("no rate");
        assert!(error.to_string().contains("'rate'"), "{error}");
    }

    #[test]
    fn a_mistyped_reference_is_reported_before_a_missing_signal() {
        // Both are wrong with the port, and the typo is the one the author can
        // act on; reporting the omission first would send them after the wrong
        // line.
        let error = ComponentType::parse(&manifest(
            "ports:\n  in:\n    requests:\n      publishes:\n        success: missing\n",
        ))
        .expect_err("unresolved");
        assert!(error.to_string().contains("missing"), "{error}");
    }

    #[test]
    fn a_port_cannot_read_the_wire_it_publishes_onto() {
        // A component publishes what it has worked out, so a port expression
        // sees the channels but not the flows; otherwise a response could be
        // defined in terms of itself.
        let error = ComponentType::parse(&manifest(
            "ports:\n  in:\n    requests:\n      publishes:\n        success: in.requests.rate\n",
        ))
        .expect_err("unresolved");
        assert!(error.to_string().contains("in"), "{error}");
    }

    #[test]
    fn a_broken_expression_is_rejected() {
        let error = ComponentType::parse(&manifest(
            "channels:\n  served:\n    unit: op/s\n    expression: 'in.requests.rate *'\n",
        ))
        .expect_err("syntax");
        assert!(error.to_string().contains("does not parse"), "{error}");
    }

    #[test]
    fn identifiers_and_names_are_shaped() {
        assert!(ComponentType::parse("id: Not Valid\nname: X\n").is_err());
        assert!(
            ComponentType::parse("id: probe\nname: X\nproperties:\n  '2bad':\n    unit: op\n")
                .is_err()
        );
    }

    #[test]
    fn constraints_are_checked_like_channels() {
        let error = ComponentType::parse(&manifest(
            "properties:\n  limit:\n    unit: op/s\nconstraints:\n  throughput:\n    demand: arriving\n    limit: limit\n",
        ))
        .expect_err("unresolved");
        assert!(error.to_string().contains("arriving"), "{error}");
    }
}
