
    // PoC for F2-VAL-001: a member `M` that republishes honest member `A`'s
    // encryption key `q` recovers A's full signing share through the
    // unconditional complaint-response flow, using only public onchain data
    // plus M's own secrets. n = 3, t = 2 (n-1 = 2 >= t).
    #[test]
    fn poc_f2_val_001_copied_q_recovers_full_share() {
        use alloy::primitives::Address;
        use k256::{Scalar, elliptic_curve::PrimeField as _};

        let mut rng = rand::thread_rng();
        let a = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        let b = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let m = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
        let (count, threshold) = (3u16, 2u16);

        let a_secrets = keygen::setup(&mut rng, a, count, threshold).unwrap();
        let b_secrets = keygen::setup(&mut rng, b, count, threshold).unwrap();
        let m_secrets = keygen::setup(&mut rng, m, count, threshold).unwrap();

        // ATTACK: M publishes its own polynomial + PoK but copies A's `q`.
        let a_commit = a_secrets.commitment();
        let b_commit = b_secrets.commitment();
        let mut m_commit = m_secrets.commitment();
        m_commit.q = a_commit.q.clone();
        assert_eq!(m_commit.q, a_commit.q, "M republished A's q");

        // The commitment (copied q, valid PoK over c) is accepted.
        let mut verified = BTreeMap::new();
        verified.insert(a, keygen::verify_commitment(a, &a_commit).unwrap());
        verified.insert(b, keygen::verify_commitment(b, &b_commit).unwrap());
        verified.insert(m, keygen::verify_commitment(m, &m_commit).unwrap());

        // Honest A and B (and M) generate and publish their encrypted shares.
        let (ss_a, share_a) =
            keygen::generate_secret_shares(a_secrets, verified.clone()).unwrap();
        let (ss_b, share_b) =
            keygen::generate_secret_shares(b_secrets, verified.clone()).unwrap();
        let (ss_m, _share_m) =
            keygen::generate_secret_shares(m_secrets, verified.clone()).unwrap();

        // --- helpers ---
        let idx = |sender: Address, target: Address| -> usize {
            let mut all = [a, b, m];
            all.sort();
            all.iter()
                .filter(|x| **x != sender)
                .position(|x| *x == target)
                .unwrap()
        };
        let ct = |share: &crate::bindings::KeyGenSecretShare, i: usize| -> [u8; 32] {
            share.f[i].to_be_bytes::<32>()
        };
        let xor = |x: [u8; 32], y: [u8; 32]| -> [u8; 32] {
            let mut o = x;
            for (p, q) in o.iter_mut().zip(y) {
                *p ^= q;
            }
            o
        };
        let scalar = |bytes: [u8; 32]| Scalar::from_repr(bytes.into()).into_option().unwrap();
        let id = |addr: Address| {
            let ser = participants::identifier(addr).serialize();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&ser);
            scalar(arr)
        };

        // Public ciphertexts observed onchain.
        let c_b_m = ct(&share_b, idx(b, m));
        let c_b_a = ct(&share_b, idx(b, a));
        let c_a_b = ct(&share_a, idx(a, b));

        // Complaint-response plaintexts M forces onto the chain (unconditional,
        // one per accused): B and A each reveal f_self(M) in the clear.
        let f_b_m = keygen::reveal_secret_share(&ss_b, m).unwrap().to_be_bytes::<32>();
        let f_a_m = keygen::reveal_secret_share(&ss_a, m).unwrap().to_be_bytes::<32>();

        // Recover the symmetric pad P_B = x(sk_B * sk_A * G), reused across
        // (B->M), (B->A) and (A->B) because q_M = q_A.
        let p_b = xor(c_b_m, f_b_m);
        let f_b_a = xor(c_b_a, p_b); // f_B(A)
        let f_a_b = xor(c_a_b, p_b); // f_A(B)

        // Cross-check the recovered f_B(A) against B's true plaintext for A.
        assert_eq!(
            f_b_a,
            keygen::reveal_secret_share(&ss_b, a).unwrap().to_be_bytes::<32>(),
            "recovered f_B(A) matches the honest plaintext",
        );

        // Interpolate degree-1 f_A through (id_B, f_A(B)), (id_M, f_A(M)) and
        // evaluate at id_A; add the other members' evaluations at A.
        let (x_a, x_b, x_m) = (id(a), id(b), id(m));
        let l_b = (x_a - x_m) * (x_b - x_m).invert().into_option().unwrap();
        let l_m = (x_a - x_b) * (x_m - x_b).invert().into_option().unwrap();
        let f_a_a = scalar(f_a_b) * l_b + scalar(f_a_m) * l_m; // f_A(A)
        let f_m_a = scalar(
            keygen::reveal_secret_share(&ss_m, a).unwrap().to_be_bytes::<32>(),
        ); // M's own
        let s_a_recovered = f_a_a + scalar(f_b_a) + f_m_a;

        // Ground truth: A's real signing share, obtained via A's honest
        // finalize (A decrypts B's share normally and takes M's share from the
        // complaint reveal, exactly as the protocol dictates).
        let gc_a = ss_a.group_commitments();
        let (_, enc_self) = keygen::verify_secret_share(gc_a, a, &share_a).unwrap();
        let vs_self = keygen::verify_encrypted_secret_share(&ss_a, a, enc_self).unwrap();
        let (_, enc_b) = keygen::verify_secret_share(gc_a, b, &share_b).unwrap();
        let vs_b = keygen::verify_encrypted_secret_share(&ss_a, b, enc_b).unwrap();
        let vs_m = keygen::verify_revealed_secret_share(
            gc_a,
            a,
            m,
            keygen::reveal_secret_share(&ss_m, a).unwrap(),
        )
        .unwrap();
        let mut a_shares = BTreeMap::new();
        a_shares.insert(a, vs_self);
        a_shares.insert(b, vs_b);
        a_shares.insert(m, vs_m);
        let key_a = keygen::finalize(ss_a, a_shares).unwrap();
        let s_a_ref = key_a.as_key_package().signing_share().to_scalar();

        assert_eq!(
            s_a_recovered, s_a_ref,
            "M recovered honest member A's complete signing share",
        );
    }
