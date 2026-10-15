//! RUN 3 (rv-val, 2026-10-08): F-VAL-060 scenario (a) at `361aaf3`, at the
//! Aegis production size (3 of 5). Child module of `crate::state::keygen`.
//!
//! `KeyGenComplained` logs emitted by an allow-listed oracle address (not the
//! coordinator) are fed through the full `apply_transition`. The validator
//! answers each with its plaintext share; the two answers plus the attacker
//! member's own legitimate share are three points of the validator's degree-2
//! polynomial, and interpolating them gives `f_me(0)` with
//! `f_me(0) * G == c_me[0]`.

use super::*;
use crate::{
    config::{Participant, ValidatorConfig},
    consensus::hashing::ConsensusDomain,
    service::Event,
};
use alloy::primitives::{U256, address};
use k256::{
    ProjectivePoint, Scalar,
    elliptic_curve::PrimeField as _,
};
use safenet_core::{
    index::{EventBlock, EventLog},
    state::{Message, StateTransition},
};

const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
const PEER3: Address = address!("90F79bf6EB2c4f870365E785982E1f101E93b906");
const ATTACKER: Address = address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65");
const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");
const ORACLE: Address = address!("00000000000000000000000000000000000BAD00");

fn nz(n: u64) -> NonZeroU64 {
    NonZeroU64::new(n).unwrap()
}

fn members() -> Vec<Participant> {
    [ME, PEER1, PEER2, PEER3, ATTACKER]
        .into_iter()
        .map(|address| Participant {
            address,
            active_from: 0,
            active_before: None,
        })
        .collect()
}

fn transition() -> Transition {
    let genesis =
        group::participants_set(&members(), group::Epoch::Genesis { salt: B256::ZERO }).unwrap();
    Transition {
        account: ME,
        genesis,
        consensus: ConsensusDomain::new(100, CONSENSUS),
        config: ValidatorConfig {
            consensus: CONSENSUS,
            staker: None,
            participants: members(),
            oracles: [ORACLE].into_iter().collect(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: nz(1440),
            key_gen_timeout: nz(120),
            signing_timeout: nz(6),
            oracle_timeout: nz(24),
        },
    }
}

fn id_scalar(address: Address) -> Scalar {
    frost_secp256k1::Identifier::derive(address.as_slice())
        .unwrap()
        .to_scalar()
}

fn scalar(value: U256) -> Scalar {
    Scalar::from_repr(value.to_be_bytes::<32>().into()).unwrap()
}

#[test]
fn forged_complaints_from_an_oracle_address_leak_the_polynomial() {
    let t = transition();
    let group = group::participants_set(
        &members(),
        group::Epoch::Number {
            consensus: CONSENSUS,
            number: nz(2),
            excluded: BTreeSet::new(),
        },
    )
    .unwrap()
    .group();
    let gid = group.id();
    let (count, threshold) = group.size();
    assert_eq!((count, threshold), (5, 3));

    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut verified = BTreeMap::new();
    for who in [ME, PEER1, PEER2, PEER3, ATTACKER] {
        let s = frost::keygen::setup(&mut rng, who, count, threshold).unwrap();
        verified.insert(who, frost::keygen::verify_commitment(who, &s.commitment()).unwrap());
        secrets.insert(who, s);
    }
    let my_c0 = secrets[&ME].commitment().c[0].clone();
    let (ss_me, _) =
        frost::keygen::generate_secret_shares(secrets[&ME].clone(), verified.clone()).unwrap();
    // What the attacker member legitimately received from ME.
    let own_point = frost::keygen::reveal_secret_share(&ss_me, ATTACKER).unwrap();

    let mut state = State {
        rollover: RolloverState::CollectingShares {
            next_epoch: EpochId::Number { number: nz(2) },
            group,
            participation: KeyGenParticipation::Participating(ss_me),
            shared: BTreeSet::new(),
            shares: BTreeMap::new(),
            complaints: BTreeMap::new(),
            deadline: Some(1_000),
        },
        active_epoch: EpochId::Number { number: nz(1) },
        ..Default::default()
    };

    // Two forged complaints (threshold - 1, so no restart), emitted by ORACLE.
    let mut points = vec![(id_scalar(ATTACKER), scalar(own_point))];
    for (i, plaintiff) in [PEER1, PEER2].into_iter().enumerate() {
        let forged = EventLog {
            block: EventBlock {
                number: 500 + i as u64,
                timestamp: 0,
            },
            index: 0,
            address: ORACLE,
            data: Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplained(
                Coordinator::KeyGenComplained {
                    gid,
                    plaintiff,
                    accused: ME,
                    compromised: false,
                },
            )),
        };
        let (next, cmds) = t.apply_transition(state, Message::Event(forged));
        state = next;
        let leaked = cmds
            .iter()
            .find_map(|c| match c {
                Command::Action(Action::KeyGenComplaintResponse {
                    plaintiff: p,
                    secret_share,
                    ..
                }) if *p == plaintiff => Some(*secret_share),
                _ => None,
            })
            .expect("plaintext response queued for a log from the oracle address");
        points.push((id_scalar(plaintiff), scalar(leaked)));
    }
    assert!(matches!(state.rollover, RolloverState::CollectingShares { .. }), "no restart");

    // Lagrange at 0 over the three points.
    let mut f0 = Scalar::ZERO;
    for (i, (xi, yi)) in points.iter().enumerate() {
        let mut num = Scalar::ONE;
        let mut den = Scalar::ONE;
        for (j, (xj, _)) in points.iter().enumerate() {
            if i != j {
                num *= -*xj;
                den *= *xi - *xj;
            }
        }
        f0 += *yi * num * den.invert().unwrap();
    }
    let point = ProjectivePoint::GENERATOR * f0;
    let encoded = {
        use k256::elliptic_curve::sec1::ToEncodedPoint as _;
        point.to_affine().to_encoded_point(false)
    };
    let x = U256::from_be_slice(&encoded.as_bytes()[1..33]);
    let y = U256::from_be_slice(&encoded.as_bytes()[33..65]);
    println!(
        "2 forged complaints from {ORACLE} answered; f_me(0)*G matches c_me[0]: {}",
        x == my_c0.x && y == my_c0.y
    );
    assert_eq!((x, y), (my_c0.x, my_c0.y), "validator's secret constant term recovered");
}
