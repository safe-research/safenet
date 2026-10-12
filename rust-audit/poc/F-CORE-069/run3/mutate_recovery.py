"""Toggle a mutation that disables `recover_authorization_gap`.

Usage: python3 -I mutate_recovery.py <storage.rs> apply|revert
"""

import sys

path, mode = sys.argv[1], sys.argv[2]
original = (
    "    pub async fn recover_authorization_gap(&self, nonce: u64) -> Result<bool, Error> {\n"
    "        let updated = sqlx::query(\n"
)
mutated = (
    "    pub async fn recover_authorization_gap(&self, nonce: u64) -> Result<bool, Error> {\n"
    "        if nonce < u64::MAX { return Ok(false); } // RV3 MUTATION\n"
    "        let updated = sqlx::query(\n"
)
with open(path, encoding="utf-8") as f:
    source = f.read()
if mode == "apply":
    assert source.count(original) == 1 and mutated not in source
    source = source.replace(original, mutated)
    assert source.count(mutated) == 1
elif mode == "revert":
    assert source.count(mutated) == 1
    source = source.replace(mutated, original)
    assert mutated not in source and source.count(original) == 1
else:
    raise SystemExit("mode must be apply or revert")
with open(path, "w", encoding="utf-8") as f:
    f.write(source)
print(f"{mode} done")
