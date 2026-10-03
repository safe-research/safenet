use super::{
    KeyGenCommitment, KeyGenConfirmation, KeyGenParticipation, RolloverState, State, Transition,
};
use crate::{consensus::epoch::EpochId, service::Effect};
use safenet_core::state::{Command, Commands};
use std::collections::BTreeMap;

impl Transition {
    /// Reaps epochs no longer needed by a signing ceremony and reconciles all
    /// process-local and persisted group secrets with the resulting state as of
    /// `block`.
    pub(super) fn handle_group_reconciliation(
        &self,
        mut state: State,
        block: u64,
    ) -> (State, Commands<State, Self>) {
        // Reap old participating epochs for which there are no more signing
        // ceremonies. This runs linearly through the entire signing state, but
        // only once per block.
        let oldest_epoch = state
            .signing
            .values()
            .map(|signing| signing.packet().epoch())
            .fold(state.active_epoch, EpochId::min);
        state.epochs = state.epochs.split_off(&oldest_epoch);

        let mut groups = state
            .epochs
            .values()
            .map(|epoch| (epoch.group.id(), Some(epoch.key_share.clone())))
            .collect::<BTreeMap<_, _>>();

        // Retain an in-progress DKG only while this validator participates in
        // it. `None` preserves any persisted material across the pre-key-share
        // phases without starting a nonce generator.
        groups.extend(match &state.rollover {
            // Already have a key share, either because our secret shares were
            // all verified or the group's rollover proposal is being signed.
            RolloverState::CollectingConfirmations {
                group,
                status: KeyGenConfirmation::Confirmed(key_share),
                ..
            }
            | RolloverState::SigningRollover {
                group,
                key_share: Some(key_share),
                ..
            } => Some((group.id(), Some(key_share.clone()))),
            // Still building our secret key share. A participating
            // commitment round is retained even while its setup is still
            // outstanding, since an earlier attempt may already have persisted
            // the group's secrets.
            RolloverState::CollectingCommitments {
                group,
                secrets: KeyGenCommitment::Participating { .. },
                ..
            }
            | RolloverState::CollectingShares {
                group,
                participation: KeyGenParticipation::Participating(_),
                ..
            }
            | RolloverState::CollectingConfirmations {
                group,
                participation: KeyGenParticipation::Participating(_),
                ..
            } => Some((group.id(), None)),
            // Any other key generation state means that we do not want to keep
            // any secrets around for that group.
            _ => None,
        });

        (
            state,
            vec![Command::Effect(Effect::ReconcileGroupSecrets {
                block,
                groups,
            })],
        )
    }
}
