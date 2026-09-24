"""Local signed OIDC fixture for engine integration tests (port of scripts/test-oidc.mjs)."""

from __future__ import annotations

import base64
import hashlib
import json
import secrets
import time
from urllib.parse import parse_qs, urlencode, urlsplit

import pyqwest
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from support import Served, serve_app


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode().rstrip("=")


class Oidc:
    def __init__(self) -> None:
        self.key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
        numbers = self.key.public_key().public_numbers()
        self.jwk = {
            "kty": "RSA",
            "n": _b64url(numbers.n.to_bytes((numbers.n.bit_length() + 7) // 8, "big")),
            "e": _b64url(numbers.e.to_bytes((numbers.e.bit_length() + 7) // 8, "big")),
            "kid": "fixture",
            "alg": "RS256",
            "use": "sig",
        }
        self.codes: dict[str, dict[str, str]] = {}
        self.issuer = ""
        self.server: Served | None = None

    @property
    def env(self) -> dict[str, str]:
        return {
            "ENGINE_OIDC_ISSUER": self.issuer,
            "ENGINE_OIDC_CLIENT_ID": "test-client",
            "ENGINE_OIDC_CLIENT_SECRET": "test-secret",
            "ENGINE_OIDC_ALLOW_HTTP": "true",
        }

    async def start(self) -> None:
        self.server = await serve_app(self)
        self.issuer = self.server.url

    async def stop(self) -> None:
        if self.server is not None:
            await self.server.close()

    async def __call__(self, scope, receive, send) -> None:
        if scope["type"] == "lifespan":
            while True:
                message = await receive()
                if message["type"] == "lifespan.startup":
                    await send({"type": "lifespan.startup.complete"})
                elif message["type"] == "lifespan.shutdown":
                    await send({"type": "lifespan.shutdown.complete"})
                    return
        path = scope["path"]
        query = {k: v[0] for k, v in parse_qs(scope["query_string"].decode()).items()}
        headers = {k.decode().lower(): v.decode() for k, v in scope["headers"]}
        body = b""
        while True:
            message = await receive()
            body += message.get("body", b"")
            if not message.get("more_body"):
                break

        async def reply(status: int, value: dict | None = None, extra: list | None = None) -> None:
            payload = json.dumps(value).encode() if value is not None else b""
            await send(
                {
                    "type": "http.response.start",
                    "status": status,
                    "headers": [(b"content-type", b"application/json"), *(extra or [])],
                }
            )
            await send({"type": "http.response.body", "body": payload})

        if path == "/.well-known/openid-configuration":
            return await reply(
                200,
                {
                    "issuer": self.issuer,
                    "authorization_endpoint": f"{self.issuer}/authorize",
                    "token_endpoint": f"{self.issuer}/token",
                    "jwks_uri": f"{self.issuer}/keys",
                },
            )
        if path == "/keys":
            return await reply(200, {"keys": [self.jwk]})
        if path == "/authorize":
            code = secrets.token_hex(24)
            self.codes[code] = query
            redirect = urlsplit(query["redirect_uri"])
            params = dict(parse_qs(redirect.query))
            params = {k: v[0] for k, v in params.items()}
            params.update({"code": code, "state": query["state"]})
            location = redirect._replace(query=urlencode(params)).geturl()
            return await reply(302, None, [(b"location", location.encode())])
        if path == "/token":
            form = {k: v[0] for k, v in parse_qs(body.decode()).items()}
            auth = self.codes.pop(form.get("code", ""), None)
            basic = "Basic " + base64.b64encode(b"test-client:test-secret").decode()
            verifier = form.get("code_verifier", "").encode()
            challenge = _b64url(hashlib.sha256(verifier).digest())
            if (
                auth is None
                or headers.get("authorization") != basic
                or auth.get("redirect_uri") != form.get("redirect_uri")
                or auth.get("code_challenge") != challenge
            ):
                return await reply(400, {"error": "invalid_grant"})
            now = int(time.time())
            head = _b64url(json.dumps({"alg": "RS256", "kid": "fixture"}).encode())
            claims = _b64url(
                json.dumps(
                    {
                        "iss": self.issuer,
                        "aud": "test-client",
                        "sub": "test-user",
                        "nonce": auth.get("nonce"),
                        "iat": now,
                        "exp": now + 300,
                    }
                ).encode()
            )
            data = f"{head}.{claims}"
            signature = self.key.sign(data.encode(), padding.PKCS1v15(), hashes.SHA256())
            return await reply(
                200,
                {
                    "id_token": f"{data}.{_b64url(signature)}",
                    "access_token": "opaque-provider-token",
                    "token_type": "Bearer",
                },
            )
        await reply(404)


async def login_management(url: str) -> str:
    """Browser-style PKCE login against the management listener; returns the session token."""
    http = pyqwest.Client(pyqwest.HTTPTransport(follow_redirects=False))
    verifier = _b64url(secrets.token_bytes(32))
    challenge = _b64url(hashlib.sha256(verifier.encode()).digest())
    login = await http.post(
        f"{url}/auth/login",
        headers={"content-type": "application/json"},
        content=json.dumps({"challenge": challenge}).encode(),
    )
    assert login.status == 200, login.content
    start = json.loads(login.content)
    authorization = await http.get(start["authorization_url"])
    assert authorization.status == 302
    callback = urlsplit(authorization.headers["location"])
    params = {k: v[0] for k, v in parse_qs(callback.query).items()}
    assert params["state"] == start["state"]
    exchange = await http.post(
        f"{url}/auth/exchange",
        headers={"content-type": "application/json"},
        content=json.dumps(
            {"state": start["state"], "code": params["code"], "verifier": verifier}
        ).encode(),
    )
    assert exchange.status == 200, exchange.content
    return json.loads(exchange.content)["access_token"]


__all__ = ["Oidc", "login_management", "serialization"]
