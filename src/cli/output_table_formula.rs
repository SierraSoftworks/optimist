use crate::domain::FormulaDefinition;

pub(super) fn render(
    revision: u64,
    formulas: &[FormulaDefinition],
) -> Result<String, human_errors::Error> {
    let rows = formulas.iter().map(|formula| {
        let unit = formula
            .compiled
            .unit
            .terms()
            .map(|(name, exponent)| format!("{name}^{exponent}"))
            .collect::<Vec<_>>()
            .join("*");
        format!(
            "{}\t{}\t{}\t{}\t{}",
            revision,
            formula.address,
            if unit.is_empty() { "1" } else { &unit },
            formula.compiled.dependencies.len(),
            formula.provenance.join("; ")
        )
    });
    Ok(
        std::iter::once("DOCUMENT_REVISION\tADDRESS\tUNIT\tDEPENDENCIES\tPROVENANCE".to_owned())
            .chain(rows)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
