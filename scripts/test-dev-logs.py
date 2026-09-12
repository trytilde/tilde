#!/usr/bin/env python3
import importlib.util
from pathlib import Path
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('dev_logs',Path(__file__).with_name('dev-logs.py'))
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
local=module.configure({})
assert local['LOGS_CLICKHOUSE_URL']=='http://127.0.0.1:18123'
assert module.configure({'DEV_LOGS_ENABLED':'0'})['TILDE_LOCAL_LOGS']=='0'
external=module.configure({'LOGS_CLICKHOUSE_URL':'https://logs.example','LOGS_CLICKHOUSE_PASSWORD':'external'})
assert external['TILDE_LOCAL_LOGS']=='0' and external['LOGS_CLICKHOUSE_PASSWORD']=='external'
assert module.configure({'TS_ADDR':'100.64.1.2'})['LOGS_CLICKHOUSE_URL']=='http://127.0.0.1:18123'
with patch.object(module.subprocess,'run') as run:
    module.start(local)
    module.start(external)
    assert run.call_count==1 and run.call_args.args[0][-1]=='logs-clickhouse'
print('Local logs configuration and startup checks passed')
