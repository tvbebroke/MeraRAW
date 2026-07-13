#!/usr/bin/env python3
"""Upload a release artifact to Cloudflare R2. Reads R2_* from environment."""
from __future__ import annotations

import os
import sys
from pathlib import Path

import boto3
from botocore.config import Config


def endpoint_url() -> str:
    raw = os.environ["R2_ENDPOINT"].rstrip("/")
    if raw.endswith("/meraraw-releases"):
        raw = raw.rsplit("/", 1)[0]
    return raw


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: r2-upload.py <local-file> [object-key]", file=sys.stderr)
        return 1

    local = Path(sys.argv[1])
    if not local.is_file():
        print(f"error: file not found: {local}", file=sys.stderr)
        return 1

    key = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("R2_OBJECT_KEY", local.name)
    bucket = os.environ["R2_BUCKET_NAME"]

    s3 = boto3.client(
        "s3",
        endpoint_url=endpoint_url(),
        aws_access_key_id=os.environ["R2_ACCESS_KEY_ID"],
        aws_secret_access_key=os.environ["R2_SECRET_ACCESS_KEY"],
        config=Config(signature_version="s3v4"),
    )

    print(f"→ Uploading {local} → s3://{bucket}/{key}")
    s3.upload_file(str(local), bucket, key)
    head = s3.head_object(Bucket=bucket, Key=key)
    print(f"✓ Uploaded ({head['ContentLength']:,} bytes)")

    print("→ Bucket contents:")
    for page in s3.get_paginator("list_objects_v2").paginate(Bucket=bucket):
        for obj in page.get("Contents", []):
            print(f"  {obj['Key']}  ({obj['Size']:,} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
