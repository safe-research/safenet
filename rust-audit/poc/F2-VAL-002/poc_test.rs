
    // PoC for F2-VAL-002: the pad is the raw x-coordinate of the shared point,
    // identical in both directions, so the two ciphertexts of a pair form a
    // two-time pad: c_{A->B} XOR c_{B->A} = m1 XOR m2, independent of the key.
    #[test]
    fn poc_f2_val_002_pad_reused_in_both_directions() {
        let alice = key(7);
        let bob = key(9);
        let m1 = [0x11u8; 32];
        let m2 = [0x22u8; 32];
        let c_ab = alice.ecdh(&bob.public_key(), m1);
        let c_ba = bob.ecdh(&alice.public_key(), m2);

        let mut lhs = c_ab;
        for (l, r) in lhs.iter_mut().zip(c_ba) {
            *l ^= r;
        }
        let mut rhs = m1;
        for (l, r) in rhs.iter_mut().zip(m2) {
            *l ^= r;
        }
        assert_eq!(lhs, rhs, "the pads cancel: the pair leaks m1 XOR m2");
    }
