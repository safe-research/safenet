#!/usr/bin/env python3
"""Apply named CA3-FIX mutation sets to a worktree (asserted, exact-text edits).

Usage: mutate.py <worktree> <set> [<set> ...]
Revert with `git checkout -- <files>`.

Sets (each undoes one fix the verified rows depend on):
  pop        F-VAL-001: no proof-of-possession check in `verified_public_key`
  droparm    F-VAL-032: re-add the arm that drops a WaitingForRequest on a foreign Sign
  warpnb     F2-VAL-003, F-SEN-003: warps no longer tick NewBlock per block (#1017)
  nonceover  F-VAL-030 replacement: a replayed GenerateNonces overwrites the stored pair
  setinput   F2-XC-001: the toml error keeps the whole document again
  noverdict  F-SEN-015: the effect handler bypasses the verdict store
  stopgap    F2-SEN-010: the #914 stopgap `<` comparison comes back (#1014 reverted)
"""
import pathlib
import sys

wt = pathlib.Path(sys.argv[1])

SETS = {
    "pop": [(
        "crates/validator/src/frost/ecdh.rs",
        """        frost_core::keys::dkg::verify_proof_of_knowledge(
            identifier,
            &public_key.as_commitment(),
            &self.proof_of_possession,
        )?;
        Ok(public_key)""",
        """        let _ = (identifier, &self.proof_of_possession); // CA3-FIX MUTATION: no PoP check
        Ok(public_key)""",
    )],
    "droparm": [(
        "crates/validator/src/state/sign.rs",
        """            Some(other) => {
                tracing::warn!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "unexpected sign event for message",
                );""",
        """            Some(SigningState::WaitingForRequest { .. }) => {} // CA3-FIX MUTATION: drop arm
            Some(other) => {
                tracing::warn!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "unexpected sign event for message",
                );""",
    )],
    "warpnb": [(
        "crates/core/src/state/mod.rs",
        """                    if warping {
                        (state, commands) =
                            self.accumulate_transition(state, commands, Message::NewBlock(block));
                    }""",
        """                    if warping && false { // CA3-FIX MUTATION: no NewBlock in warps
                        (state, commands) =
                            self.accumulate_transition(state, commands, Message::NewBlock(block));
                    }""",
    )],
    "nonceover": [(
        "crates/validator/src/secrets/store.rs",
        "             ON CONFLICT (signature_id) DO NOTHING\",",
        "             ON CONFLICT (signature_id) DO UPDATE SET nonces = excluded.nonces /* CA3-FIX MUTATION */\",",
    )],
    "setinput": [(
        "crates/core/src/config.rs",
        "        inner.set_input(None);",
        "        // CA3-FIX MUTATION: inner.set_input(None);",
    )],
    "noverdict": [
        (
            "crates/sentinel/src/effect.rs",
            "        match self.verdicts.get(request_id, block).await {",
            "        match Ok::<Option<CheckOutcome>, crate::verdicts::Error>(None) { // CA3-FIX MUTATION",
        ),
        (
            "crates/sentinel/src/effect.rs",
            "        match self.verdicts.record(request_id, block, outcome).await {",
            "        match Ok::<CheckOutcome, crate::verdicts::Error>(outcome) { // CA3-FIX MUTATION",
        ),
    ],
    "stopgap": [(
        "crates/sentinel/src/service.rs",
        """                if block <= *commit_deadline {
                    return true;
                }""",
        """                if block < *commit_deadline { // CA3-FIX MUTATION: #914 stopgap
                    return true;
                }""",
    )],
}

for name in sys.argv[2:]:
    for rel, old, new in SETS[name]:
        f = wt / rel
        text = f.read_text()
        assert text.count(old) == 1, f"{name}: expected one match in {rel}, got {text.count(old)}"
        text = text.replace(old, new)
        f.write_text(text)
        assert f.read_text().count(new) == 1
        print(f"applied {name} to {rel}")
