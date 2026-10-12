"""Make loopback-only sentinel configs from the shipped sample, changing only rpc and database."""
import sys

sample, outdir, port = sys.argv[1], sys.argv[2], sys.argv[3]
s = open(sample).read()
old_rpc = 'rpc = "https://rpc.gnosischain.com"'
old_db = 'database = "sqlite:/var/lib/safenet/sentinel/data/storage.db"'
assert s.count(old_rpc) == 1 and s.count(old_db) == 1
for name, window in (("sample", None), ("window0", "0")):
    t = s.replace(old_rpc, f'rpc = "http://127.0.0.1:{port}"')
    t = t.replace(old_db, f'database = "sqlite:{outdir}/{name}.db?mode=rwc"')
    t = t.replace('log_filter = "info"', 'log_filter = "debug"')
    if window is not None:
        assert t.count("voting_window = 6") == 1
        t = t.replace("voting_window = 6", f"voting_window = {window}")
    open(f"{outdir}/{name}.toml", "w").write(t)
    print("wrote", f"{outdir}/{name}.toml")
