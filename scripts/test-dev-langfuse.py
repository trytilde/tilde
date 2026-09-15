#!/usr/bin/env python3
"""Exercise local defaults, config precedence, and readiness without Docker."""
import contextlib
import importlib.util
import io
import json
import tempfile
from pathlib import Path
from unittest.mock import patch
spec = importlib.util.spec_from_file_location('dev_langfuse',Path(__file__).with_name('dev-langfuse.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
with tempfile.TemporaryDirectory() as directory:
    module.ROOT=Path(directory)
    first=module.configure({})
    assert first['LANGFUSE_BASE_URL']=='http://127.0.0.1:3003'
    assert not list(module.ROOT.iterdir())
    second=module.configure({'TS_ADDR':'100.64.12.34'})
    assert first['LANGFUSE_SECRET_KEY']==second['LANGFUSE_SECRET_KEY']
    assert second['LANGFUSE_PUBLIC_URL']=='http://100.64.12.34:3003'
    assert second['LANGFUSE_BASE_URL']==f"http://{second['LANGFUSE_DEV_ADDRESS']}:3003"
    overrides=module.configure({'DEV_LANGFUSE_PUBLIC_KEY':'local-public','DEV_LANGFUSE_SECRET_KEY':'local-secret','LANGFUSE_DEV_LOGIN_PASSWORD':'local-password','LANGFUSE_DB_PASSWORD':'local-db'})
    assert overrides['TILDE_LOCAL_LANGFUSE']=='1'
    assert overrides['LANGFUSE_PUBLIC_KEY']=='local-public'
    assert overrides['LANGFUSE_SECRET_KEY']=='local-secret'
    assert overrides['LANGFUSE_DB_PASSWORD']=='local-db'
    external={'LANGFUSE_BASE_URL':'https://langfuse.example','LANGFUSE_PUBLIC_KEY':'external-public','LANGFUSE_SECRET_KEY':'external-secret'}
    configured=module.configure(external)
    assert configured['TILDE_LOCAL_LANGFUSE']=='0'
    assert configured['LANGFUSE_SECRET_KEY']=='external-secret'
    assert module.configure({'DEV_LANGFUSE_ENABLED':'0'})['TILDE_LOCAL_LANGFUSE']=='0'
    assert module.configure({'LANGFUSE_BASE_URL':'https://partial.example'})['TILDE_LOCAL_LANGFUSE']=='0'
    class Response(io.BytesIO):
        status=200
    def request(*args,**kwargs):
        return Response(json.dumps({'data':[{'id':'local-project'}]}).encode())
    output=io.StringIO()
    with patch.object(module.subprocess,'run') as run,patch.object(module.urllib.request,'urlopen',side_effect=request),contextlib.redirect_stdout(output):
        module.start(overrides)
        assert len(run.call_args_list)==3
        assert run.call_args_list[0].args[0][-1]=='minio'
        assert 'langfuse-bucket' in run.call_args_list[1].args[0]
        assert 'langfuse-worker' in run.call_args_list[2].args[0]
    assert first['LANGFUSE_SECRET_KEY'] not in output.getvalue()
    assert overrides['LANGFUSE_DEV_LOGIN_PASSWORD'] not in output.getvalue()
    assert overrides['LANGFUSE_SECRET_KEY'] not in output.getvalue()
    # Exercise both readiness and authenticated API requests with the TS_ADDR binding.
    requested=[]
    def tailscale_request(value, **kwargs):
        requested.append(value if isinstance(value, str) else value.full_url)
        return request(value, **kwargs)
    with patch.object(module.subprocess,'run'),patch.object(module.urllib.request,'urlopen',side_effect=tailscale_request),contextlib.redirect_stdout(output):
        module.start(second)
    assert requested==['http://100.64.12.34:3003/api/public/health','http://100.64.12.34:3003/api/public/projects']
    with patch.object(module.subprocess,'run'),patch.object(module.time,'monotonic',side_effect=[0,241]):
        try:module.start(first)
        except RuntimeError:pass
        else:raise AssertionError('Readiness failure must stop dev startup')
print('Langfuse dev bootstrap checks passed')
