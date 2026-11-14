/// Full historical fact, including source checkpoint, is voted on as one value.
pub(crate) type BalanceProposal = holderproof_types::BalanceFact;

/// Select any quorum, regardless of arrival order. A node that submits more
/// than once is excluded entirely so conflicting votes cannot count twice.
pub(crate) fn agreeing_proposal<Signer: PartialEq>(
    proposals: &[(Signer, BalanceProposal)],
    threshold: usize,
) -> Option<&BalanceProposal> {
    if threshold == 0 {
        return None;
    }
    let unique: Vec<_> = proposals
        .iter()
        .filter(|(signer, _)| {
            proposals
                .iter()
                .filter(|(other, _)| other == signer)
                .count()
                == 1
        })
        .collect();
    unique.iter().find_map(|(_, candidate)| {
        let votes = unique
            .iter()
            .filter(|(_, proposal)| proposal == candidate)
            .count();
        (votes >= threshold).then_some(candidate)
    })
}

pub(crate) fn valid_committee<Signer: PartialEq>(nodes: &[Signer]) -> bool {
    nodes.len() == holderproof_types::COMMITTEE_SIZE
        && nodes
            .iter()
            .enumerate()
            .all(|(index, node)| !nodes[..index].contains(node))
}
