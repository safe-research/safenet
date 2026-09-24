
#[test]
fn qa_q6b_array_length_preallocation_vm() {
    use alloy::sol_types::SolValue;
    fn vm_kb() -> (u64, u64) {
        let s = std::fs::read_to_string("/proc/self/statm").unwrap();
        let mut it = s.split_whitespace();
        let size: u64 = it.next().unwrap().parse().unwrap();
        let rss: u64 = it.next().unwrap().parse().unwrap();
        (size * 4, rss * 4)
    }
    for bits in [20u32, 24, 26, 28, 30, 32, 34, 40, 63, 64] {
        let n: u128 = 1u128 << bits;
        let mut w = [0u8; 32];
        w[32 - ((bits / 8) as usize) - 1..].copy_from_slice(&n.to_be_bytes()[16 - ((bits / 8) as usize) - 1..]);
        let hex = format!(
            "0000000000000000000000000000000000000000000000000000000000000020{}",
            alloy::hex::encode(w)
        );
        let data = alloy::hex::decode(&hex).unwrap();
        let (vs0, rs0) = vm_kb();
        let r = <Vec<alloy::primitives::U256>>::abi_decode(&data);
        let (vs1, rs1) = vm_kb();
        println!(
            "Q6b 2^{bits} (={} elems, {} MiB requested) -> {:?} | dVSZ={}kB dRSS={}kB",
            n, n * 32 / 1048576,
            r.as_ref().map(|v| v.len()).map_err(|e| e.to_string()),
            vs1 as i64 - vs0 as i64, rs1 as i64 - rs0 as i64
        );
    }
}
