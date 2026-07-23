mod support;

use std::str::FromStr;

use optimist::{
    command::{CommandRequest, CreateNode, GraphCommand},
    domain::{Edge, EdgeId, EntityId},
    project::ProjectCatalog,
};
use proptest::prelude::*;
use uuid::Uuid;

use support::{edge, entity_id, node, observation, project_id, valid_endpoints};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn ids_and_edge_ids_round_trip(
        project in project_id(),
        entity in entity_id(),
        edge in edge(),
    ) {
        prop_assert_eq!(project.to_string().parse(), Ok(project));
        prop_assert_eq!(EntityId::from_str(&entity.to_string()), Ok(entity));
        let edge_id = edge.id();
        prop_assert_eq!(EdgeId::from_str(&edge_id.to_string()), Ok(edge_id));
    }

    #[test]
    fn constrained_endpoints_always_construct_valid_edges(
        endpoints in valid_endpoints(),
    ) {
        let (source, source_kind, destination, destination_kind, payload) = endpoints;
        let expected_kind = payload.kind();
        let edge = Edge::new(source, source_kind, destination, destination_kind, payload);
        prop_assert!(edge.is_ok());
        prop_assert_eq!(edge.expect("checked valid edge").id().kind, expected_kind);
    }

    #[test]
    fn core_aggregates_round_trip_through_json(
        node in node(),
        edge in edge(),
        observation in observation(),
    ) {
        let node_json = serde_json::to_vec(&node).expect("serialize node");
        let edge_json = serde_json::to_vec(&edge).expect("serialize edge");
        let observation_json = serde_json::to_vec(&observation).expect("serialize observation");

        let decoded_node = serde_json::from_slice(&node_json).expect("deserialize node");
        let decoded_edge = serde_json::from_slice(&edge_json).expect("deserialize edge");
        let decoded_observation =
            serde_json::from_slice(&observation_json).expect("deserialize observation");
        prop_assert_eq!(node, decoded_node);
        prop_assert_eq!(edge, decoded_edge);
        prop_assert_eq!(observation, decoded_observation);
    }

    #[test]
    fn project_command_histories_are_isolated(seed in any::<u64>(), template in node()) {
        let mut catalog = ProjectCatalog::new();
        let left = catalog.create("Left project".to_owned()).expect("create left project");
        let right = catalog.create("Right project".to_owned()).expect("create right project");
        let request_id = Uuid::from_u128(u128::from(seed));

        let left_request = create_request(request_id, 0, &template, "left");
        let right_request = create_request(request_id, 0, &template, "right");
        catalog.execute(&left.id, left_request).expect("execute left command");
        catalog.execute(&right.id, right_request).expect("execute right command");

        let left_nodes = catalog.list_nodes(&left.id).expect("list left nodes");
        let right_nodes = catalog.list_nodes(&right.id).expect("list right nodes");
        prop_assert_eq!(left_nodes.len(), 1);
        prop_assert_eq!(right_nodes.len(), 1);
        prop_assert!(left_nodes[0].name.starts_with("left_"));
        prop_assert!(right_nodes[0].name.starts_with("right_"));
    }

    #[test]
    fn command_sequences_and_retries_are_deterministic(seed in any::<u64>(), count in 1_usize..8) {
        let mut left = ProjectCatalog::new();
        let mut right = ProjectCatalog::new();
        let left_project = left.create("Project".to_owned()).expect("create left catalog project");
        let right_project = right.create("Project".to_owned()).expect("create right catalog project");

        for revision in 0..count {
            let request_id = Uuid::from_u128((u128::from(seed) << 64) | revision as u128);
            let request = generated_create_request(request_id, revision as u64, seed, revision);
            let left_result = left.execute(&left_project.id, request.clone()).expect("execute left");
            let retry_result = left.execute(&left_project.id, request.clone()).expect("retry left");
            let right_result = right.execute(&right_project.id, request).expect("execute right");
            prop_assert_eq!(&left_result, &retry_result);
            prop_assert_eq!(&left_result, &right_result);
        }

        prop_assert_eq!(
            left.list_nodes(&left_project.id).expect("list left nodes"),
            right.list_nodes(&right_project.id).expect("list right nodes"),
        );
    }
}

fn create_request(
    request_id: Uuid,
    revision: u64,
    template: &optimist::domain::Node,
    prefix: &str,
) -> CommandRequest {
    CommandRequest {
        request_id,
        expected_revision: revision,
        command: GraphCommand::CreateNode(CreateNode {
            name: format!("{prefix}_{}", template.name),
            title: template.title.clone(),
            payload: template.payload.clone(),
        }),
    }
}

fn generated_create_request(
    request_id: Uuid,
    revision: u64,
    seed: u64,
    index: usize,
) -> CommandRequest {
    CommandRequest {
        request_id,
        expected_revision: revision,
        command: GraphCommand::CreateNode(CreateNode {
            name: format!("node_{seed}_{index}"),
            title: format!("Node {index}"),
            payload: optimist::domain::NodePayload::Factor(optimist::domain::Factor {
                controllable: index.is_multiple_of(2),
                evidence: Vec::new(),
            }),
        }),
    }
}
