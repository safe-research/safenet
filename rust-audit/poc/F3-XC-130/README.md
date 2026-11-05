# F3-XC-130 PoC (QA3-CORE)

The sentinel binary built at `361aaf3` with `rpc` set to `htps://gnosis-mainnet.example.invalid/v2/QA3-FAKE-API-KEY-0123456789` (literal, `sentinel-literal.toml`), to `${QA3_RPC_URL}` with the same kind of URL in the environment (`sentinel-env.toml`), and an `https://127.0.0.1:1/...` control (`sentinel-control.toml`).

## Run

```sh
for v in literal env control; do
  QA3_RPC_URL='htps://gnosis-mainnet.example.invalid/v2/QA3-ENV-SECRET-KEY-9876' \
    target/debug/sentinel --config-file sentinel-$v.toml; echo "exit status: $?"
done
```

Bite check: `fix-and-test.full.patch` rejects schemes other than `http`, `https`, `ws` and `wss` in `Provider::connect` (remediation 1) and adds a unit test that the key is in neither `Display` nor `Debug` of the error; `test.patch` is the test alone.

## Files

- `sentinel-binary-at-361aaf3.txt`, `sentinel-binary-with-fix.txt`
- `unit-test-at-361aaf3.txt` (fails: the key is printed), `unit-test-with-fix.txt`, `core-suite-with-fix.txt`
