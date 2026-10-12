    // rv-xc run 3: F2-XC-001 variant. Temporary test pasted at the end of `mod tests` in
    // `crates/core/src/config.rs` and reverted with `git checkout --` after the run.
    //
    // `ParseError::new` (the #1046 fix) slices `&contents[..span.start]`. A `toml` error span
    // that starts inside a multi-byte character would make that slice panic, and a `str` slice
    // panic message quotes up to 256 bytes of the document, which can hold the signer key.
    // This test feeds `parse` many malformed documents that contain multi-byte characters and
    // checks that no call panics and that every reported span starts on a character boundary.
    mod rv_xc_span_fuzz {
        use super::*;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        const BASE: &str = "rpc = \"https://rpc.example/v3/apikey\" # é€🦀\n\
                            signer = \"0xSECRET\"\n\
                            count = 1 # ünïcödé ❤\n\
                            k = \"€🦀é\\u00e9\"\n\
                            [t] # ß\n\
                            ä = 'é€'\n";

        const EDITS: &[&str] = &[
            "\"", "'", "=", "[", "]", "\\", "#", "\n", "x", "0", ".", "é", "€", "🦀", "\u{0}",
            "\t", "{", "}", ",", "\"\"\"", "'''", "\r", "\\u", "\\U0001", "ä=", "=é", "[é]",
            "é.é", "0x", "1e", "+", "-", ":", "T", "\u{7f}", "\u{feff}", "\u{200b}",
        ];

        struct Stats {
            cases: u64,
            errors: u64,
            panics: u64,
            off_boundary: u64,
        }

        fn check(doc: &str, stats: &mut Stats) {
            stats.cases += 1;
            // The span `toml` reports, before `ParseError::new` sees it.
            if let Err(err) = toml::from_str::<Config>(doc) {
                stats.errors += 1;
                if let Some(span) = err.span() {
                    if !doc.is_char_boundary(span.start.min(doc.len())) {
                        stats.off_boundary += 1;
                        println!("OFF-BOUNDARY span {span:?} in {doc:?}");
                    }
                }
            }
            // The production function itself.
            if catch_unwind(AssertUnwindSafe(|| parse::<Config>(doc).err().map(|e| e.to_string())))
                .is_err()
            {
                stats.panics += 1;
                println!("PANIC in parse() for {doc:?}");
            }
        }

        #[test]
        fn rv_xc_parse_never_slices_inside_a_character() {
            let base = BASE.replace("SECRET", SECRET);
            let mut stats = Stats { cases: 0, errors: 0, panics: 0, off_boundary: 0 };
            let boundaries: Vec<usize> = base
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(base.len()))
                .collect();

            // Every single-edit document: an insertion at every boundary, a deletion of every
            // character, and a replacement of every character.
            for &pos in &boundaries {
                for edit in EDITS {
                    check(&format!("{}{}{}", &base[..pos], edit, &base[pos..]), &mut stats);
                }
            }
            for (i, c) in base.char_indices() {
                let after = i + c.len_utf8();
                check(&format!("{}{}", &base[..i], &base[after..]), &mut stats);
                for edit in EDITS {
                    check(&format!("{}{}{}", &base[..i], edit, &base[after..]), &mut stats);
                }
            }

            // Random multi-edit documents (xorshift64, fixed seed, so the run is reproducible).
            let mut state: u64 = 0x5afe_0000_0000_0001;
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            for _ in 0..200_000 {
                let mut doc = base.clone();
                for _ in 0..(1 + next() % 4) {
                    let bounds: Vec<usize> = doc
                        .char_indices()
                        .map(|(i, _)| i)
                        .chain(std::iter::once(doc.len()))
                        .collect();
                    let pos = bounds[(next() % bounds.len() as u64) as usize];
                    if next() % 3 == 0 && pos < doc.len() {
                        let c = doc[pos..].chars().next().unwrap();
                        doc.replace_range(pos..pos + c.len_utf8(), "");
                    } else {
                        let edit = EDITS[(next() % EDITS.len() as u64) as usize];
                        doc.insert_str(pos, edit);
                    }
                }
                check(&doc, &mut stats);
            }

            println!(
                "rv-xc span fuzz: {} documents, {} parse errors, {} spans off a character boundary, {} panics in parse()",
                stats.cases, stats.errors, stats.off_boundary, stats.panics
            );
            assert_eq!(stats.panics, 0);
            assert_eq!(stats.off_boundary, 0);
        }
    }
