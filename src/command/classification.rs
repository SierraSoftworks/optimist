use super::GraphCommand;

impl GraphCommand {
    pub(crate) const fn changes_graph(&self) -> bool {
        matches!(
            self,
            Self::CreateNode(_)
                | Self::DeleteNode(_)
                | Self::UpdateNodeMetadata(_)
                | Self::SetNodeQuantityState(_)
                | Self::CreateEvidence(_)
                | Self::UpdateEvidence(_)
                | Self::DeleteEvidence(_)
                | Self::CreateEdge(_)
                | Self::DeleteEdge(_)
                | Self::UpdateEdgeMetadata(_)
                | Self::AppendObservation(_)
                | Self::CorrectObservation(_)
                | Self::SetMeasurementCalibration(_)
                | Self::SetEffectProfile(_)
                | Self::SetSquiggleEstimate(_)
                | Self::RemoveEstimate(_)
        )
    }
}
